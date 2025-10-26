use esp_radio::wifi::ScanConfig;
use microjson::JSONValue;
use reqwless::{
    client::{HttpClient, TlsConfig},
    request::RequestBuilder,
};

use embassy_net::{
    Runner, Stack,
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{ModeConfig, WifiController, WifiDevice, WifiEvent, WifiStaState};
use log::{error, info};

use crate::{
    get_capy_config,
    ui::{CapyState, get_capy_state},
};
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

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

    loop {
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

        let auth_header = format!("token {}", token);

        let headers = [
            ("Authorization", auth_header.as_str()),
            ("User-Agent", "ESP32-Client"),
        ];

        let mut http_req = client
            .request(
                reqwless::request::Method::GET,
                "https://cappycoding.koyeb.app/metrics/prs?user=suri-codes&state=open&per_page=5",
            )
            .await
            .unwrap()
            .headers(&headers);

        let response = match http_req.send(&mut buffer).await {
            Ok(o) => o,
            Err(e) => {
                error!("{e:?}");
                continue;
            }
        };

        info!("Got response");
        let res = response.body().read_to_end().await.unwrap();

        let content = core::str::from_utf8(res).unwrap();

        let prs = parse_pull_requests(content);
        info!("pr's: {:#?}", prs);

        drop(http_req);

        info!("making new request");
        let mut http_req = match client
            .request(
                reqwless::request::Method::GET,
                "https://cappycoding.koyeb.app/metrics/workflows?user=suri-codes&per_page=5",
            )
            .await
        {
            Ok(o) => o.headers(&headers),
            Err(_) => continue,
        };

        let response = http_req.send(&mut buffer).await.unwrap();
        info!("Got response");
        let res = response.body().read_to_end().await.unwrap();

        let content = core::str::from_utf8(res).unwrap();

        let workflows = parse_workflows(content);
        info!("workflows: {:#?}", workflows);
        drop(http_req);

        info!("making commits request");
        let mut http_req = client
            .request(
                reqwless::request::Method::GET,
                // "https://cappycoding.koyeb.app/metrics/commits?user=suri-codes&since=2025-10-24T00:00:00Z",
                "https://cappycoding.koyeb.app/metrics/commits?user=suri-codes",
            )
            .await
            .unwrap()
            .headers(&headers);

        let response = http_req.send(&mut buffer).await.unwrap();
        info!("Got response");
        let res = response.body().read_to_end().await.unwrap();

        let content = core::str::from_utf8(res).unwrap();

        let commit_data = parse_commits(content);
        info!("commits: {:#?}", commit_data);

        let mut state = get_capy_state().lock().await;

        *state = Some(CapyState {
            commits: commit_data,
            pr: prs,
            workflow: workflows,
        });

        drop(state);

        Timer::after_secs(60).await;
    }
}

#[derive(Debug)]
pub struct CommitData {
    pub total: u32,
}

fn parse_commits(buffer: &str) -> CommitData {
    let json = JSONValue::load(buffer);
    let x = json.get_key_value("total").unwrap().read_integer().unwrap();

    CommitData { total: x as u32 }
}

#[derive(Debug)]
pub struct WorkflowData {
    pub name: String,
    pub status: String,
    pub conclusion: String,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}

fn parse_workflows(buffer: &str) -> Vec<WorkflowData> {
    let json = JSONValue::load(buffer);
    let workflows_array = json.iter_array().unwrap();

    let mut workflows = Vec::new();

    for workflow in workflows_array {
        let name = workflow
            .get_key_value("name")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let status = workflow
            .get_key_value("status")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let conclusion = workflow
            .get_key_value("conclusion")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let html_url = workflow
            .get_key_value("htmlUrl")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let created_at = workflow
            .get_key_value("createdAt")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let updated_at = workflow
            .get_key_value("updatedAt")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();

        workflows.push(WorkflowData {
            name,
            status,
            conclusion,
            html_url,
            created_at,
            updated_at,
        });
    }

    workflows
}

#[derive(Debug)]
pub struct PullRequestData {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub url: String,
    pub updated_at: String,
    pub author: String,
}

fn parse_pull_requests(buffer: &str) -> Vec<PullRequestData> {
    let json = JSONValue::load(buffer);
    let prs_array = json.iter_array().unwrap();

    let mut prs = Vec::new();

    for pr in prs_array {
        let number = pr.get_key_value("number").unwrap().read_integer().unwrap() as u32;
        let title = pr
            .get_key_value("title")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let state = pr
            .get_key_value("state")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let url = pr
            .get_key_value("url")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let updated_at = pr
            .get_key_value("updatedAt")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();
        let author = pr
            .get_key_value("author")
            .unwrap()
            .read_string()
            .unwrap()
            .to_owned();

        prs.push(PullRequestData {
            number,
            title,
            state,
            url,
            updated_at,
            author,
        });
    }

    prs
}
