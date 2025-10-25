use bt_hci::controller::ExternalController;
use esp_hal::peripherals;
use esp_radio::{Controller, ble::controller::BleConnector};
use log::info;
use trouble_host::{HostResources, prelude::DefaultPacketPool};

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

type Type = HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>;

#[embassy_executor::task]
pub async fn ble_task(radio: &'static Controller<'static>, bt: peripherals::BT<'static>) {
    info!("BLE task started!");
    let transport = BleConnector::new(radio, bt, Default::default()).unwrap();
    let ble_controller: ExternalController<BleConnector<'_>, 20> =
        ExternalController::<_, 20>::new(transport);
    let mut resources: Type = HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);
}
