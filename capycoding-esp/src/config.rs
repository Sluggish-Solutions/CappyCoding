use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_storage::{ReadStorage, nor_flash::NorFlash, nor_flash::check_write};
use esp_bootloader_esp_idf::partitions::{self, PartitionEntry};
use esp_storage::FlashStorage;
use heapless::String;
use log::{error, info};
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CapyConfig {
    pub api_tokens: Tokens,
    pub wifi_credentials: WifiCredentials,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WifiCredentials {
    pub ssid: String<30>,
    pub password: String<30>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub github: String<30>,
}

pub type CapyConfigHandle = &'static Mutex<CriticalSectionRawMutex, Option<CapyConfig>>;
impl CapyConfig {
    pub fn load(flash: &mut FlashStorage<'static>) -> Option<Self> {
        let mut pt_mem = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
        let nvs_entry = get_nvs_partition(flash, &mut pt_mem);
        let mut nvs = nvs_entry.as_embedded_storage(flash);
        let nvs_read_offset = 0;
        let mut raw_config_buf = [0u8; size_of::<CapyConfig>()];
        nvs.read(nvs_read_offset, &mut raw_config_buf).unwrap();

        // Check if flash is empty (uninitialized)
        let is_empty = raw_config_buf.iter().all(|&b| b == 0xFF || b == 0x00);

        if is_empty {
            info!("Flash is empty, returning default CapyState");
            // return CapyConfig::default();
            return None;
        }

        match postcard::from_bytes(&raw_config_buf) {
            Ok(read_from_flash) => {
                info!("Successfully loaded CapyState from flash");
                Some(read_from_flash)
            }
            Err(e) => {
                error!("Error deserializing from flash: {e}, returning default");
                // CapyConfig::default()
                None
            }
        }
    }

    pub fn write(&self, flash: &mut FlashStorage<'static>) {
        let mut pt_mem = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
        let nvs_entry = get_nvs_partition(flash, &mut pt_mem);
        let mut nvs = nvs_entry.as_embedded_storage(flash);
        let nvs_write_offset = 0;

        // Serialize self to buffer
        let mut raw_config_buf = [0u8; size_of::<CapyConfig>()];
        let serialized = postcard::to_slice(self, &mut raw_config_buf).unwrap();

        // Align the write length to word boundary (4 bytes for ESP32)
        let aligned_len = (serialized.len() + 3) & !3;

        // Erase before writing - REQUIRED for flash
        let erase_size = (aligned_len + 0xFFF) & !0xFFF; // Round up to 4KB sector
        nvs.erase(nvs_write_offset, erase_size as u32)
            .map_err(|e| error!("Failed to erase flash! {:?}", e))
            .unwrap();

        check_write(&nvs, nvs_write_offset, aligned_len).unwrap();
        nvs.write(nvs_write_offset, &raw_config_buf[..aligned_len])
            .map_err(|e| error!("Failed to write flash! {e}"))
            .unwrap();

        info!(
            "Successfully wrote CapyState to flash ({} bytes)",
            aligned_len
        );
    }

    pub fn erase(&self, flash: &mut FlashStorage<'static>) {
        let mut pt_mem = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
        let nvs_entry = get_nvs_partition(flash, &mut pt_mem);
        let mut nvs = nvs_entry.as_embedded_storage(flash);
        let nvs_write_offset = 0;

        // Serialize self to buffer
        let mut raw_config_buf = [0u8; size_of::<CapyConfig>()];
        let serialized = postcard::to_slice(self, &mut raw_config_buf).unwrap();

        // Align the write length to word boundary (4 bytes for ESP32)
        let aligned_len = (serialized.len() + 3) & !3;

        // Erase before writing - REQUIRED for flash
        let erase_size = (aligned_len + 0xFFF) & !0xFFF; // Round up to 4KB sector
        nvs.erase(nvs_write_offset, erase_size as u32)
            .map_err(|e| error!("Failed to erase flash! {:?}", e))
            .unwrap();
    }
}

fn get_nvs_partition<'a>(
    flash: &mut FlashStorage<'static>,
    pt_mem: &'a mut [u8; partitions::PARTITION_TABLE_MAX_LEN],
) -> PartitionEntry<'a> {
    let pt = partitions::read_partition_table(flash, pt_mem).unwrap();

    for i in 0..pt.len() {
        let raw = pt.get_partition(i).unwrap();
        info!("pt i:{i}, value: {raw:?}");
    }

    pt.find_partition(partitions::PartitionType::Data(
        partitions::DataPartitionSubType::Nvs,
    ))
    .unwrap()
    .unwrap()
}
