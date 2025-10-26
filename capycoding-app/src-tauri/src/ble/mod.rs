use anyhow::anyhow;
use anyhow::Result;
use ble_types::PERIPHERAL_NAME;
use log::info;

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

    pub async fn send_config_data(&mut self) -> Result<()> {
        todo!()
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

    per.ok_or(anyhow!("couldnt find trouble!"))
}
