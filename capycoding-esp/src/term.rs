use alloc::boxed::Box;
use display_interface_spi::SPIInterface;
use embassy_time::Timer;
use embedded_hal_bus::spi::ExclusiveDevice;
use log::info;
use weact_studio_epd::{
    WeActStudio290BlackWhiteDriver,
    graphics::{Display, Display290BlackWhite, DisplayRotation},
};

use mousefood::{EmbeddedBackend, EmbeddedBackendConfig, fonts};
use ratatui::Terminal;

use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    peripherals::{GPIO0, GPIO1, GPIO2, GPIO3},
    spi::master::Spi,
};

use crate::{get_capy_config, ui::root_draw};

pub type CapyDisplay = Display<128, 296, 4736, weact_studio_epd::Color>;

pub type CapyTerm<'a> = Terminal<
    EmbeddedBackend<'a, Display<128, 296, 4736, weact_studio_epd::Color>, weact_studio_epd::Color>,
>;

/// pin configuration for weact term
pub struct WeactTermInitPins {
    pub cs_pin: GPIO3<'static>,
    pub dc_pin: GPIO2<'static>,
    pub rst_pin: GPIO1<'static>,
    pub busy_pin: GPIO0<'static>,
}

/// Initializes the weact terminal
pub fn setup_weact_term<'a>(
    spi: Spi<'static, Blocking>,
    display: &'a mut CapyDisplay,
    pins: WeactTermInitPins,
) -> CapyTerm<'a> {
    let cs = Output::new(pins.cs_pin, Level::High, OutputConfig::default());
    let busy = Input::new(pins.busy_pin, InputConfig::default().with_pull(Pull::Up));
    let dc = Output::new(pins.dc_pin, Level::Low, OutputConfig::default());
    let rst = Output::new(pins.rst_pin, Level::High, OutputConfig::default());
    let delay = Delay::new();

    let spi_device = ExclusiveDevice::new(spi, cs, delay.clone()).unwrap();

    let spi_interface = SPIInterface::new(spi_device, dc);

    let mut driver = WeActStudio290BlackWhiteDriver::new(spi_interface, busy, rst, delay.clone());
    display.set_rotation(DisplayRotation::Rotate90);
    driver.init().unwrap();

    let config = EmbeddedBackendConfig {
        font_regular: fonts::MONO_10X20,
        flush_callback: Box::new(move |d| {
            driver.fast_update(d).unwrap();
        }),
        ..Default::default()
    };

    let backend = EmbeddedBackend::new(display, config);

    Terminal::new(backend).unwrap()
}

/// render task for capycode
#[embassy_executor::task]
pub async fn ui_task(spi: Spi<'static, Blocking>, term_init_pins: WeactTermInitPins) {
    info!("UI task started!");
    let mut display = Display290BlackWhite::new();
    let mut term = setup_weact_term(spi, &mut display, term_init_pins);
    loop {
        let config = get_capy_config().lock().await;
        // let config: embassy_sync::mutex::MutexGuard<
        //     '_,
        //     embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        //     Option<crate::CapyConfig>,
        // > = config_ref.lock().await;

        term.draw(|f| root_draw(f, config)).unwrap();
        Timer::after_millis(5).await;
    }
}
