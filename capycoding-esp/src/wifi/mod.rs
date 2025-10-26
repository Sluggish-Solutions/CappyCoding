use esp_radio::wifi::ScanConfig;
use reqwless::{
    client::{HttpClient, TlsConfig},
    request::RequestBuilder,
};

use embassy_net::{
    Config, Runner, Stack, StackResources,
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_time::{Duration, Timer};
use esp_hal::peripherals;
use esp_radio::{
    Controller,
    wifi::{ModeConfig, WifiController, WifiDevice, WifiEvent, WifiStaState},
};
use log::info;

use crate::{CapyConfigHandle, get_capy_config};
use alloc::{format, string::String};

const SSID: &str = "Surendra S21";
const PASSWORD: &str = "Surendra2006";

mod api;

#[embassy_executor::task]
pub async fn wifi_task(stack: Stack<'static>, tls_seed: u64) {
    wait_for_connection(stack).await;

    access_website(stack, tls_seed).await;
}

#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());

    loop {
        if esp_radio::wifi::sta_state() == WifiStaState::Connected {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }

        if !matches!(controller.is_started(), Ok(true)) {
            info!("Starting WiFi controller...");

            let ssid: String = SSID.into();
            let password: String = PASSWORD.into();

            let wifi_config = esp_radio::wifi::ClientConfig::default()
                .with_failure_retry_cnt(5)
                .with_ssid(ssid)
                .with_password(password);

            let mode_client = ModeConfig::Client(wifi_config);
            controller.set_config(&mode_client).unwrap();
            controller.start_async().await.unwrap();
        }

        info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => {
                info!("Wifi connected!");
                // Connection successful, continue the loop
            }
            Err(e) => {
                info!("Failed to connect to wifi: {e:?}");
                info!("Performing network scan to find available networks...");

                // Perform scan on failure
                let scan_config = ScanConfig::default();

                match controller.scan_with_config_async(scan_config).await {
                    Ok(scan_results) => {
                        info!("=== WiFi Networks Found ===");
                        for (idx, ap) in scan_results.iter().enumerate() {
                            info!(
                                "{}. SSID: {:?}, Channel: {}, RSSI: {}, Auth: {:?}",
                                idx + 1,
                                ap.ssid,
                                ap.channel,
                                ap.signal_strength,
                                ap.auth_method
                            );
                        }
                        info!("=== End of scan results ===");
                    }
                    Err(scan_err) => {
                        info!("Scan also failed: {:?}", scan_err);
                    }
                }

                // Wait before retrying connection
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

async fn wait_for_connection(stack: Stack<'_>) {
    info!("Waiting for link to be up");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

async fn access_website(stack: Stack<'_>, tls_seed: u64) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let dns = DnsSocket::new(stack);
    let tcp_state = TcpClientState::<1, 4096, 4096>::new();
    let tcp = TcpClient::new(stack, &tcp_state);

    let tls = TlsConfig::new(
        tls_seed,
        &mut rx_buffer,
        &mut tx_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut client: HttpClient<'_, TcpClient<'_, 1, 4096, 4096>, DnsSocket<'_>> =
        HttpClient::new_with_tls(&tcp, &dns, tls);

    let mut buffer = [0u8; 4096];

    let token = loop {
        let config = get_capy_config().lock().await;

        match config
            .as_ref()
            .and_then(|c| Some(c.api_tokens.github.clone()))
        {
            Some(token) => break token.clone(),
            None => {
                info!("Waiting for github token!");
                Timer::after(Duration::from_millis(500)).await
            }
        }
    };
    // let token = {
    //     while let (config) = get_capy_config().lock().await {
    //         Timer::after(Duration::from_millis(500)).await;
    //     }

    //     let config = get_capy_config().lock().await;

    //     config
    //         .as_ref()
    //         .and_then(|c| Some(c.api_tokens.github.clone()))
    //         .expect("GitHub token not configured")
    // };

    let auth_header = format!("token {}", token);

    let headers = [
        ("Authorization", auth_header.as_str()),
        ("User-Agent", "ESP32-Client"),
    ];

    let mut http_req = client
        .request(
            reqwless::request::Method::GET,
            "https://cappycoding.koyeb.app/metrics/prs?user=suri-codes&state=open&per_page=3",
        )
        .await
        .unwrap()
        .headers(&headers);

    let response = http_req.send(&mut buffer).await.unwrap();
    info!("Got response");
    let res = response.body().read_to_end().await.unwrap();

    let content = core::str::from_utf8(res).unwrap();
    info!("pr's: {}", content);

    // drop(http_req);

    // info!("making new request");
    // let mut http_req = client
    //     .request(
    //         reqwless::request::Method::GET,
    //         "http://localhost:8080/metrics/workflows?user=suri-codes&per_page=1",
    //     )
    //     .await
    //     .unwrap()
    //     .headers(&headers);

    // let response = http_req.send(&mut buffer).await.unwrap();
    // info!("Got response");
    // let res = response.body().read_to_end().await.unwrap();

    // let content = core::str::from_utf8(res).unwrap();
    // info!("workflows: {}", content);
    drop(http_req);

    info!("making commits request");
    let mut http_req = client
        .request(
            reqwless::request::Method::GET,
            "http://localhost:8080/metrics/commits?user=suri-codes&since=2025-10-24T00:00:00Z",
        )
        .await
        .unwrap()
        .headers(&headers);

    let response = http_req.send(&mut buffer).await.unwrap();
    info!("Got response");
    let res = response.body().read_to_end().await.unwrap();

    let content = core::str::from_utf8(res).unwrap();
    info!("commits: {}", content);
}
