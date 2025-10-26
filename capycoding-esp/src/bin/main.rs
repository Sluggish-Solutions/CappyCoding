#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use capycoding_esp::ble::ble_task;
use capycoding_esp::wifi::{connection, net_task, wifi_task};
use capycoding_esp::{WeactTermInitPins, init_capy_config, ui_task};
use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, StackResources};
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::spi::{self, master::Spi};
use esp_hal::{time::Rate, timer::timg::TimerGroup};
use esp_radio::wifi::Config as WifiConfig;
use esp_storage::FlashStorage;

use esp_backtrace as _;

use log::info;
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static RADIO: StaticCell<esp_radio::Controller<'static>> = StaticCell::new();

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

    init_capy_config(&mut flash);

    let (wifi_controller, ifaces) =
        esp_radio::wifi::new(radio, peripherals.WIFI, WifiConfig::default()).unwrap();

    let wifi_interface = ifaces.sta;

    let rng = Rng::new();
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);
    let tls_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    let config = embassy_net::Config::dhcpv4(dhcp_config);
    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        net_seed,
    );

    // BLE handler
    spawner.spawn(ble_task(radio, peripherals.BT)).unwrap();

    // UI handler
    spawner.spawn(ui_task(spi_bus, term_init_pins)).unwrap();

    // wifi util tasks
    spawner.spawn(connection(wifi_controller)).unwrap();
    spawner.spawn(net_task(runner)).unwrap();

    // main wifi task
    spawner.spawn(wifi_task(stack, tls_seed)).unwrap();
}
