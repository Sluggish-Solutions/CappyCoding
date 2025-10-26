use embassy_net::{Config, StackResources};
use esp_hal::peripherals;
use esp_radio::{Controller, wifi::ModeConfig};
use log::info;

use crate::CapyConfigHandle;
use alloc::string::String;

const SSID: &str = "Albert iphone 14";
const PASSWORD: &str = "alb12345";

// const SOCKS: u32 = 4;
// static RESOURCES: static_cell::StaticCell<StackResources<SOCKS>> = static_cell::StaticCell::new();

#[embassy_executor::task]
pub async fn wifi_task(
    radio: &'static Controller<'static>,
    wifi: peripherals::WIFI<'static>,
    config_handle: CapyConfigHandle,
) {
    info!("WIFI task started!");
    let (mut _wifi_controller, _interfaces) = esp_radio::wifi::new(radio, wifi, Default::default())
        .expect("Failed to initialize Wi-Fi controller");

    // let ssid: String = SSID.into();
    // let password: String = PASSWORD.into();
    // let mut wifi_config = esp_radio::wifi::ClientConfig::default()
    //     .with_ssid(ssid)
    //     .with_password(password);

    // let mode_client = ModeConfig::Client(wifi_config);

    // _wifi_controller
    //     .set_config(&mode_client)
    //     .expect("failed to get wifi config");

    // // Start WiFi
    // info!("Starting WiFi...");
    // _wifi_controller.start().expect("Failed to start WiFi");
    // info!("WiFi started!");

    // // Connect to WiFi
    // info!("Connecting to WiFi SSID: {}", SSID);
    // _wifi_controller
    //     .connect()
    //     .expect("Failed to connect to WiFi");
    // info!("WiFi connected!");

    // let netconfig = Config::dhcpv4(Default::default());
    // let resources: &'static mut StackResources<4> = RESOURCES.init(StackResources::new());
    // let (stack, runner) = embassy_net::new(_wifi_interfaces.sta, netconfig, resources, seed);

    // spawner.spawn(net_task(runner)).ok();
    // info!("Waiting for IP address...");
    // stack.wait_config_up().await;

    // if let Some(config) = stack.config_v4() {
    //     info!("Got IP address: {}", config.address);
    // }
}
