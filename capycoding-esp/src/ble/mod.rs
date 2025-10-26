use ble_types::{PERIPHERAL_ADVERTISEMENT, PERIPHERAL_NAME};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::Timer;
use esp_hal::peripherals;
use esp_radio::Controller as RadioController;
use esp_radio::ble::controller::BleConnector;
use log::{info, warn};
use trouble_host::{HostResources, prelude::DefaultPacketPool};

use embassy_futures::join::join;
use embassy_futures::select::select;
#[allow(unused_imports)]
use trouble_host::prelude::*;

use crate::CapyConfigHandle;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

#[gatt_server]
struct Server {
    config_service: ConfigService,
}

#[gatt_service(uuid = ble_types::CONFIG_SERVICE_UUID)]
struct ConfigService {
    #[characteristic(uuid = ble_types::WIFI_SSID_CHARACTERISTIC, write, read, notify)]
    wifi_ssid: heapless::Vec<u8, 24>,

    #[characteristic(uuid = ble_types::WIFI_PASSWORD_CHARACTERISTIC, write, read, notify)]
    wifi_password: heapless::Vec<u8, 24>,

    #[characteristic(uuid = ble_types::GITHUB_TOKEN_CHARACTERISTIC, write, read, notify)]
    github_token: heapless::Vec<u8, 24>,
}

type CapyResources = HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>;

#[embassy_executor::task]
pub async fn ble_task(
    radio: &'static RadioController<'static>,
    bt: peripherals::BT<'static>,
    config_handle: CapyConfigHandle,
) {
    info!("BLE task started!");
    let transport = BleConnector::new(radio, bt, Default::default()).unwrap();
    let ble_controller: ExternalController<BleConnector<'_>, 20> =
        ExternalController::<_, 20>::new(transport);
    // let mut resources: CapyResources = HostResources::new();
    // let _stack = trouble_host::new(ble_controller, &mut resources);

    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(ble_controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: PERIPHERAL_NAME,
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    let _ = join(ble_co_task(runner), async {
        loop {
            match advertise(PERIPHERAL_ADVERTISEMENT, &mut peripheral, &server).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't run when no one is connected.
                    let a = gatt_events_task(&server, &conn);
                    let b = custom_task(&server, &conn, &stack);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    select(a, b).await;
                }
                Err(e) => {
                    panic!("[adv] error: {:?}", e);
                }
            }
        }
    })
    .await;
}

async fn ble_co_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let ssid = &server.config_service.wifi_ssid;
    let passwd = &server.config_service.wifi_password;
    let gh_token = &server.config_service.github_token;

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    GattEvent::Read(event) => {
                        if event.handle() == ssid.handle {
                            let value = server.get(ssid);
                            info!("[gatt] Read Event to ssid Characteristic: {:?}", value);
                        }

                        if event.handle() == passwd.handle {
                            let value = server.get(passwd);
                            info!("[gatt] Read Event to passwd Characteristic: {:?}", value);
                        }
                    }
                    GattEvent::Write(event) => {
                        if event.handle() == ssid.handle {
                            info!(
                                "[gatt] Write Event to ssid Characteristic: {:?}",
                                event.data()
                            );
                        }

                        if event.handle() == passwd.handle {
                            info!(
                                "[gatt] Write Event to gh_token Characteristic: {:?}",
                                event.data()
                            );

                            let x = heapless::Vec::from_slice(event.data()).unwrap();

                            server.set(passwd, &x).unwrap();
                        }
                    }
                    _ => {}
                };
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[gatt] error sending response: {:?}", e),
                };
            }
            _ => {} // ignore other Gatt Connection Events
        }
    };
    info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];

    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[[0x0f, 0x18], [0xdf, 0x13]]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;

    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
}

/// Example task to use the BLE notifier interface.
/// This task will notify the connected central of a counter value every 2 seconds.
/// It will also read the RSSI value every 2 seconds.
/// and will stop when the connection is closed by the central or an error occurs.
async fn custom_task<C: Controller, P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    stack: &Stack<'_, C, P>,
) {
    let mut tick: u8 = 0;
    // let level = server.battery_service.level;
    // let string = &server.string_service.string;
    loop {
        // let slice = string.get(server).unwrap();
        // let new_string = str::from_utf8(slice.as_slice()).unwrap();
        // info!("value of string: {}", new_string);
        tick = tick.wrapping_add(1);
        info!("[custom_task] notifying connection of tick {}", tick);
        // if level.notify(conn, &tick).await.is_err() {
        //     info!("[custom_task] error notifying connection");
        //     break;
        // };

        // if string.notify(conn, &slice).await.is_err() {
        //     info!("[custom_task] error notifying connection");
        //     break;
        // }
        // read RSSI (Received Signal Strength Indicator) of the connection.
        if let Ok(rssi) = conn.raw().rssi(stack).await {
            info!("[custom_task] RSSI: {:?}", rssi);
        } else {
            info!("[custom_task] error getting RSSI");
            break;
        };
        Timer::after_secs(2).await;
    }
}
