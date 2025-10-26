#![no_std]

mod term;
use esp_storage::FlashStorage;
pub use term::*;

mod config;
pub use config::*;

pub mod ble;
pub mod wifi;

pub mod ui;

pub extern crate alloc;

// static GLOBAL_STATE: Lazy<Arc<Mutex<CriticalSectionRawMutex, String>> =
//     Lazy::new(|| Mutex::new(String::new()));
use alloc::string::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use once_cell::sync::OnceCell;

// Your state structure
#[derive(Default)]
pub struct AppState {
    pub counter: u32,
    pub data: Option<String>,
    // Add your fields here
}

static CONFIG: OnceCell<Mutex<CriticalSectionRawMutex, Option<CapyConfig>>> = OnceCell::new();

// Initialize the singleton (call once at startup)
pub fn init_capy_config(flash: &mut FlashStorage<'static>) {
    let state = CapyConfig::load(flash);
    CONFIG.get_or_init(|| Mutex::new(state));
}

// Get a reference to the singleton
pub fn get_capy_config() -> &'static Mutex<CriticalSectionRawMutex, Option<CapyConfig>> {
    CONFIG
        .get()
        .expect("State not initialized! Call init_state() first")
}
