#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use bt_hci::controller::ExternalController;
use capycoding_esp::WeactTermInitPins;
use capycoding_esp::widgets::root_draw;
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::spi::{self, master::Spi};
use esp_hal::{Blocking, clock::CpuClock, time::Rate, timer::timg::TimerGroup};
use esp_radio::ble::controller::BleConnector;
use log::info;
use trouble_host::prelude::*;
use weact_studio_epd::graphics::Display290BlackWhite;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> () {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 66320);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 200 * 1024);

    // * RTOS init *
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    // * WIFI *
    let radio_init = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut _wifi_controller, _interfaces) =
        esp_radio::wifi::new(&radio_init, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    // * BLE *
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(&radio_init, peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 20>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);

    // * INIT TERM *//
    let mosi_pin = peripherals.GPIO5;
    let sclk_pin = peripherals.GPIO4;

    let spi_bus: Spi<'_, esp_hal::Blocking> = Spi::new(
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

    // * RENDER TASK * //
    spawner.spawn(render(spi_bus, term_init_pins)).unwrap();
}

#[embassy_executor::task]
/// render task for capycode
async fn render(spi: Spi<'static, Blocking>, term_init_pins: WeactTermInitPins) {
    info!("Render Task Initiated!");
    let mut display = Display290BlackWhite::new();
    let mut term = capycoding_esp::setup_weact_term(spi, &mut display, term_init_pins);
    loop {
        term.draw(|f| root_draw(f)).unwrap();
    }
}
