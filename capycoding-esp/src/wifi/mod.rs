use esp_hal::peripherals;
use esp_radio::Controller;
use log::info;

use crate::CapyConfigHandle;

#[embassy_executor::task]
pub async fn wifi_task(
    radio: &'static Controller<'static>,
    wifi: peripherals::WIFI<'static>,
    config_handle: CapyConfigHandle,
) {
    info!("WIFI task started!");
    let (mut _wifi_controller, _interfaces) = esp_radio::wifi::new(radio, wifi, Default::default())
        .expect("Failed to initialize Wi-Fi controller");
}
