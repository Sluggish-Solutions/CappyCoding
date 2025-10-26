#![no_std]

mod term;
pub use term::*;

mod config;
pub use config::*;

pub mod ble;
pub mod wifi;

pub mod ui;

pub extern crate alloc;
