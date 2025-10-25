#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use bt_hci::controller::ExternalController;
use capycoding_esp::alloc::fmt::format;
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    spi::{self, master::Spi},
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_radio::ble::controller::BleConnector;
use log::info;
use ratatui::{
    Frame,
    style::{Style, Stylize},
    widgets::{Block, Paragraph, Wrap},
};
use trouble_host::prelude::*;
use weact_studio_epd::graphics::Display290BlackWhite;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 0.6.0

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

    // TODO: Spawn some tasks
    let _ = spawner;

    let mut display = Display290BlackWhite::new();

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

    // let mut terminal = capycoding_esp::setup_terminal(peripherals, &mut display);
    let mut terminal = capycoding_esp::setup_weact_term(
        spi_bus,
        &mut display,
        peripherals.GPIO3,
        peripherals.GPIO2,
        peripherals.GPIO1,
        peripherals.GPIO0,
    );

    info!("Terminal initialized!");

    // Run an infinite loop, where widgets will be rendered
    let mut i = 0;
    loop {
        terminal
            .draw(|f| {
                draw(f, &mut i)
                // f.render_widget(draw(), area);
            })
            .unwrap();

        i += 1;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.1/examples/src/bin
}

#[embassy_executor::task]
async fn render() {}

fn draw(frame: &mut Frame, i: &mut i32) {
    let text = format(format_args!("Ratatui on embedded devices! {i}"));
    let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
    let bordered_block = Block::bordered()
        .border_style(Style::new().yellow())
        .title("Mousefood");
    frame.render_widget(paragraph.block(bordered_block), frame.area());
}

// fn setup_terminal(peripherals: &mut Peripherals) -> Terminal<EmbeddedBackend<'_>> {
//     let mosi_pin = peripherals.GPIO5;
//     let sclk_pin = peripherals.GPIO4;
//     let cs_pin = peripherals.GPIO3;
//     let dc_pin = peripherals.GPIO2;
//     let rst_pin = peripherals.GPIO1;
//     let busy_pin = peripherals.GPIO0;

//     let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
//     let busy = Input::new(busy_pin, InputConfig::default().with_pull(Pull::Up));
//     let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
//     let rst = Output::new(rst_pin, Level::High, OutputConfig::default());
//     let delay = Delay::new();

//     let spi_bus = Spi::new(
//         peripherals.SPI2,
//         spi::master::Config::default()
//             .with_frequency(Rate::from_khz(100))
//             .with_mode(spi::Mode::_0),
//     )
//     .unwrap()
//     .with_sck(sclk_pin)
//     .with_mosi(mosi_pin);

//     let spi_device = ExclusiveDevice::new(spi_bus, cs, delay.clone()).unwrap();
//     let spi_interface = SPIInterface::new(spi_device, dc);

//     let mut driver = WeActStudio290BlackWhiteDriver::new(spi_interface, busy, rst, delay.clone());
//     let mut display = Display290BlackWhite::new();
//     display.set_rotation(DisplayRotation::Rotate90);
//     driver.init().unwrap();

//     let config = EmbeddedBackendConfig {
//         font_regular: fonts::MONO_10X20,
//         flush_callback: Box::new(move |d| {
//             driver.fast_update(d).unwrap();
//         }),
//         ..Default::default()
//     };

//     let backend = EmbeddedBackend::new(&mut display, config);
//     Terminal::new(backend).unwrap()
// }
