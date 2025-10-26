use anyhow::anyhow;
use anyhow::Result;
use ble_types::CONFIG_SERVICE_UUID;
use ble_types::GITHUB_TOKEN_CHARACTERISTIC;
use ble_types::PERIPHERAL_NAME;
use ble_types::WIFI_PASSWORD_CHARACTERISTIC;
use ble_types::WIFI_SSID_CHARACTERISTIC;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::bleuuid::BleUuid;
use log::info;
use uuid::Uuid;

use std::time::Duration;
use tokio::time;

use btleplug::api::{Central, Manager as _, Peripheral as PeripheralTrait, ScanFilter};

use btleplug::platform::Manager;
use btleplug::platform::Peripheral;

#[derive(Default, Debug, Clone)]
pub struct CapyCoder {
    peripheral: Option<Peripheral>,
}

impl CapyCoder {
    pub async fn connect(&mut self) -> Result<()> {
        let capycoder_perif = get_peripheral().await?;

        capycoder_perif.connect().await?;

        self.peripheral = Some(capycoder_perif);

        info!("connection to capycoder successful!");

        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        let Some(ref perf) = self.peripheral else {
            return Ok(());
        };

        Ok(perf.disconnect().await?)
    }

    pub async fn send_config_data(
        &mut self,
        wifi_name: String,
        wifi_pass: String,
        github_token: String,
    ) -> Result<()> {
        let Some(ref capy) = self.peripheral else {
            return Err(anyhow!("not connected to the thing"));
        };

        capy.discover_services().await?;
        let services = capy.services();
        let Some(target_service) = services
            .iter()
            .find(|e| e.uuid == uuid_from_u16(CONFIG_SERVICE_UUID))
        else {
            return Err(anyhow!("unable to find target service uuid"));
        };
        info!("target service: {target_service:#?}");

        let target_characteristic = target_service
            .characteristics
            .iter()
            .find(|e| e.uuid == uuid_from_u16(GITHUB_TOKEN_CHARACTERISTIC))
            .ok_or(anyhow!("was not able to find characteristic uuid"))?;

        capy.write(
            target_characteristic,
            github_token.as_bytes(),
            btleplug::api::WriteType::WithoutResponse,
        )
        .await?;

        let target_characteristic = target_service
            .characteristics
            .iter()
            .find(|e| e.uuid == uuid_from_u16(WIFI_SSID_CHARACTERISTIC))
            .ok_or(anyhow!("was not able to find characteristic uuid"))?;

        capy.write(
            target_characteristic,
            wifi_name.as_bytes(),
            btleplug::api::WriteType::WithoutResponse,
        )
        .await?;

        let target_characteristic = target_service
            .characteristics
            .iter()
            .find(|e| e.uuid == uuid_from_u16(WIFI_PASSWORD_CHARACTERISTIC))
            .ok_or(anyhow!("was not able to find characteristic uuid"))?;

        capy.write(
            target_characteristic,
            wifi_pass.as_bytes(),
            btleplug::api::WriteType::WithoutResponse,
        )
        .await?;

        Ok(())
    }
}

async fn get_peripheral() -> Result<Peripheral> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters.first().ok_or(anyhow!("No adapter found!!"))?;

    println!("Starting scan on {}...", adapter.adapter_info().await?);
    adapter
        .start_scan(ScanFilter::default())
        .await
        .expect("Can't scan BLE adapter for connected devices...");

    time::sleep(Duration::from_secs(5)).await;
    let peripherals = adapter.peripherals().await?;
    let mut per = None;
    for e in peripherals.into_iter() {
        let props = e.properties().await.unwrap().unwrap();
        if props
            .local_name
            .unwrap_or("unknown".to_owned())
            .contains(PERIPHERAL_NAME)
        {
            per = Some(e);
            break;
        }
    }

    per.ok_or(anyhow!("couldnt find capy!"))
}
