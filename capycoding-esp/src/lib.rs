#![no_std]

use alloc::boxed::Box;
use display_interface_spi::SPIInterface;
use embedded_hal_bus::spi::ExclusiveDevice;
use weact_studio_epd::{
    WeActStudio290BlackWhiteDriver,
    graphics::{Display, DisplayRotation},
};

use mousefood::{EmbeddedBackend, EmbeddedBackendConfig, fonts};
use ratatui::Terminal;

use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{self, Input, InputConfig, Level, Output, OutputConfig, Pull},
    spi::master::Spi,
};
pub extern crate alloc;

pub type CapyDisplay = Display<128, 296, 4736, weact_studio_epd::Color>;

pub type CapyTerm<'a> = Terminal<
    EmbeddedBackend<'a, Display<128, 296, 4736, weact_studio_epd::Color>, weact_studio_epd::Color>,
>;

pub fn setup_weact_term<'a, CS, DC, RST, BUSY>(
    spi: Spi<'static, Blocking>,
    display: &'a mut CapyDisplay,
    cs_pin: CS,
    dc_pin: DC,
    rst_pin: RST,
    busy_pin: BUSY,
) -> CapyTerm<'a>
where
    CS: gpio::OutputPin + 'static,
    DC: esp_hal::gpio::OutputPin + 'static,
    RST: esp_hal::gpio::OutputPin + 'static,
    BUSY: esp_hal::gpio::InputPin + 'static,
{
    let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
    let busy = Input::new(busy_pin, InputConfig::default().with_pull(Pull::Up));
    let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
    let rst = Output::new(rst_pin, Level::High, OutputConfig::default());
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

    let term = Terminal::new(backend).unwrap();

    term
}
