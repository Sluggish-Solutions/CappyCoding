#![no_std]

use heapless::String;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

// pub const CONFIG_SERVICE_UUID: Uuid = uuid!("171f7d49-bd79-4e85-9bbd-9e0c57191e56");
// pub const CONFIG_SERVICE_UUID_STR: &str = "171f7d49-bd79-4e85-9bbd-9e0c57191e56";
pub const CONFIG_SERVICE_UUID: u16 = 0xbeef;
pub const WIFI_SSID_CHARACTERISTIC: u16 = 0xbeed;
pub const WIFI_PASSWORD_CHARACTERISTIC: u16 = 0xbeee;
pub const GITHUB_TOKEN_CHARACTERISTIC: u16 = 0xbeea;

pub const PERIPHERAL_NAME: &str = "CapyCoder";
pub const PERIPHERAL_ADVERTISEMENT: &str = PERIPHERAL_NAME;

// pub const WIFI_CREDENTIAL_CHARACTERISTIC: Uuid = uuid!("ab2f0d66-306f-4735-9af3-35930eeb31ca");
pub const TOKENS_CHARACTERISTIC: Uuid = uuid!("361c1911-a3b1-4935-ae72-2ffc828099a1");

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WifiCredentials {
    pub ssid: String<30>,
    pub password: String<30>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub github: String<50>,
}
