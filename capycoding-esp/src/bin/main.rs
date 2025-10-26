#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use capycoding_esp::ble::ble_task;
use capycoding_esp::wifi::wifi_task;
use capycoding_esp::{CapyConfig, WeactTermInitPins, ui_task};
use embassy_executor::Spawner;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{self, TcpClient, TcpClientState};
use embassy_net::{Config, DhcpConfig, Runner, Stack, StackResources};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::spi::{self, master::Spi};
use esp_hal::{time::Rate, timer::timg::TimerGroup};
use esp_radio::wifi::{
    self, ClientConfig, Config as WifiConfig, ModeConfig, WifiController, WifiDevice, WifiEvent,
    WifiStaState,
};
use esp_storage::FlashStorage;

use esp_backtrace as _;

use capycoding_esp::alloc::string::String;
use log::info;
use reqwless::client::{HttpClient, TlsConfig};
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

use embassy_sync::mutex::Mutex;

const SSID: &str = "akcasa Guest";
const PASSWORD: &str = "Chigerian123";

pub static CONFIG: StaticCell<Mutex<CriticalSectionRawMutex, Option<CapyConfig>>> =
    StaticCell::new();

static RADIO: StaticCell<esp_radio::Controller<'static>> = StaticCell::new();
static PUB_SUB_CHANNEL: static_cell::StaticCell<PubSubChannel<NoopRawMutex, Message, 20, 3, 1>> =
    static_cell::StaticCell::new();

#[derive(Copy, Clone)]
pub enum Message {
    Connected,
}

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}
#[esp_rtos::main]
async fn main(spawner: Spawner) -> () {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // shit ton of memory alloc
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 66320);
    esp_alloc::heap_allocator!(size: 170* 1024);

    // rtos init
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    // init radio
    let radio_init: esp_radio::Controller<'_> =
        esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");

    let radio = RADIO.init(radio_init);

    // init for term
    let mosi_pin = peripherals.GPIO5;
    let sclk_pin = peripherals.GPIO4;

    let spi_bus = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(100))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(sclk_pin)
    .with_mosi(mosi_pin);

    let term_init_pins = WeactTermInitPins {
        cs_pin: peripherals.GPIO3,
        dc_pin: peripherals.GPIO2,
        rst_pin: peripherals.GPIO1,
        busy_pin: peripherals.GPIO0,
    };

    let mut flash = FlashStorage::new(peripherals.FLASH);

    let state = CapyConfig::load(&mut flash);

    // spawn tasks
    let capyconfig = CONFIG.init(Mutex::new(state));

    let capy_ref = &*capyconfig;

    spawner
        .spawn(ble_task(radio, peripherals.BT, capy_ref))
        .unwrap();

    // spawner
    //     .spawn(wifi_task(radio, peripherals.WIFI, capy_ref))
    //     .unwrap();
    spawner
        .spawn(ui_task(spi_bus, term_init_pins, capy_ref))
        .unwrap();

    let (wifi_controller, ifaces) =
        esp_radio::wifi::new(radio, peripherals.WIFI, WifiConfig::default()).unwrap();

    let wifi_interface = ifaces.sta;

    let rng = Rng::new();
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);
    let tls_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    // dhcp_config.hostname = Some(String::from_str("implRust").unwrap());

    let config = embassy_net::Config::dhcpv4(dhcp_config);
    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        net_seed,
    );

    

    spawner.spawn(connection(wifi_controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    wait_for_connection(stack).await;
    access_website(stack, tls_seed).await;

    // let esp_wifi_ctrl = &*mk_static!(
    //     Esp<'static>,
    //     esp_wifi::init(timer1.timer0, rng.clone(), peripherals.RADIO_CLK,).unwrap()
    // );
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        if esp_radio::wifi::sta_state() == WifiStaState::Connected {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }

        if !matches!(controller.is_started(), Ok(true)) {
            // let client_config = Config::dhcpv4(ClientConfig {
            //     ssid: SSID.try_into().unwrap(),
            //     password: PASSWORD.try_into().unwrap(),
            //     ..Default::default()
            // });
            // let client_config =ModeConfig::Client(ClientConfig::default().with_ssid("lol".to_owned()));
            let ssid: String = SSID.into();
            let password: String = PASSWORD.into();

            let wifi_config = esp_radio::wifi::ClientConfig::default()
                .with_ssid(ssid)
                .with_password(password);

            let mode_client = ModeConfig::Client(wifi_config);

            controller.set_config(&mode_client).unwrap();

            // controller.set_configuration(&client_config).unwrap();
            // println!("Starting wifi");
            controller.start_async().await.unwrap();
            // println!("Wifi started!");
        }
        info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => info!("Wifi connected!"),
            Err(e) => {
                info!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

async fn wait_for_connection(stack: Stack<'_>) {
    info!("Waiting for link to be up");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

async fn access_website(
    // stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>
    stack: Stack<'_>

//     stack: &Stack<esp_radio::wifi::WifiDevice>,
    


    , tls_seed: u64) {

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut buffer = [0u8; 4096];
    let mut http_req = client
        .request(
            reqwless::request::Method::GET,
            // "https://jsonplaceholder.typicode.com/posts/1",
"https://cappycoding.koyeb.app/"
        )
        .await
        .unwrap();
    let response = http_req.send(&mut buffer).await.unwrap();

    info!("Got response");
    let res = response.body().read_to_end().await.unwrap();

    let content = core::str::from_utf8(res).unwrap();
    info!("{}", content);
}
