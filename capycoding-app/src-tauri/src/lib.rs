use crate::types::{Id, Info};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn call_api() {
    println!("I was invoked from JavaScript!");
}

#[taurpc::procedures(export_to = "../src/types.ts")]
trait Api {
    async fn hello_world() -> String;

    async fn get_user(id: Id) -> Info;
}

#[derive(Clone)]
struct ApiImpl;

#[taurpc::resolvers]
impl Api for ApiImpl {
    async fn hello_world(self) -> String {
        "Penis".to_owned()
    }

    async fn get_user(self, id: Id) -> Info {
        Info {
            id,
            first_name: "Surendra".to_owned(),
            last_name: "Jammishetti".to_owned(),
        }
    }
}

mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(taurpc::create_ipc_handler(ApiImpl.into_handler()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
