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

    fetch_github_metrics(stack, tls_seed).await;
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

async fn fetch_github_metrics(stack: Stack<'_>, tls_seed: u64) {
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

    const BASE_URL: &str = "https://cappycoding.koyeb.app/metrics";
    // const USER: &str = "akhil-datla";
    const USER: &str = "suri-codes";

    // Macro to reduce repetitive request code
    macro_rules! fetch {
        ($url:expr, $buffer:expr, $headers:expr) => {{
            let http_req = match client.request(reqwless::request::Method::GET, $url).await {
                Ok(req) => Some(req.headers($headers)),
                Err(e) => {
                    error!("Request creation failed for {}: {:?}", $url, e);
                    None
                }
            };

            match http_req {
                Some(mut req) => match req.send($buffer).await {
                    Ok(response) => match response.body().read_to_end().await {
                        Ok(res) => match core::str::from_utf8(res) {
                            Ok(content) => Some(content),
                            Err(e) => {
                                error!("Invalid UTF-8 in response from {}: {:?}", $url, e);
                                None
                            }
                        },
                        Err(e) => {
                            error!("Failed to read response body for {}: {:?}", $url, e);
                            None
                        }
                    },
                    Err(e) => {
                        error!("Request failed for {}: {:?}", $url, e);
                        None
                    }
                },
                None => None,
            }
        }};
    }

    loop {
        // Wait for token
        let token = loop {
            let config = get_capy_config().lock().await;
            if let Some(ref config) = *config {
                let x = config.api_tokens.github.clone();
                break x;
            }
            drop(config);
            info!("Waiting for github token!");
            Timer::after(Duration::from_millis(500)).await;
        };

        let auth_header = format!("token {}", token);
        let headers = [
            ("Authorization", auth_header.as_str()),
            ("User-Agent", "ESP32-Client"),
        ];

        let mut buffer = [0u8; 2048];

        // Fetch PRs
        info!("Fetching PRs...");
        let prs_url = format!("{}/prs?user={}&per_page=5", BASE_URL, USER);
        let prs = match fetch!(&prs_url, &mut buffer, &headers) {
            Some(content) => {
                let parsed = parse_pull_requests(content);
                info!("PRs: {:#?}", parsed);
                parsed
            }
            None => {
                error!("Failed to fetch PRs, retrying in 60s");
                Timer::after_secs(60).await;
                continue;
            }
        };

        // Fetch workflows
        info!("Fetching workflows...");
        let workflows_url = format!("{}/workflows?user={}&per_page=5", BASE_URL, USER);
        let workflows = match fetch!(&workflows_url, &mut buffer, &headers) {
            Some(content) => {
                let parsed = parse_workflows(content);
                info!("Workflows: {:#?}", parsed);
                parsed
            }
            None => {
                error!("Failed to fetch workflows, retrying in 60s");
                Timer::after_secs(60).await;
                continue;
            }
        };

        // Fetch commits - all time
        info!("Fetching all-time commits...");
        let all_time_url = format!("{}/commits?user={}", BASE_URL, USER);
        let all_time = match fetch!(&all_time_url, &mut buffer, &headers) {
            Some(content) => content.to_owned(),
            None => {
                error!("Failed to fetch all-time commits, retrying in 60s");
                Timer::after_secs(60).await;
                continue;
            }
        };

        // Fetch commits - last week
        info!("Fetching last week commits...");
        let last_week_url = format!(
            "{}/commits?user={}&since=2025-10-19T00:00:00Z",
            BASE_URL, USER
        );
        let last_week = match fetch!(&last_week_url, &mut buffer, &headers) {
            Some(content) => content.to_owned(),
            None => {
                error!("Failed to fetch last week commits, retrying in 60s");
                Timer::after_secs(60).await;
                continue;
            }
        };

        // Fetch commits - last month
        info!("Fetching last month commits...");
        let last_month_url = format!(
            "{}/commits?user={}&since=2025-09-26T00:00:00Z",
            BASE_URL, USER
        );
        let last_month = match fetch!(&last_month_url, &mut buffer, &headers) {
            Some(content) => content.to_owned(),
            None => {
                error!("Failed to fetch last month commits, retrying in 60s");
                Timer::after_secs(60).await;
                continue;
            }
        };

        let commit_data = parse_commits(&all_time, &last_month, &last_week);
        info!("Commits: {:#?}", commit_data);

        // Update state
        {
            let mut state = get_capy_state().lock().await;
            let carousel_index = state.as_ref().map(|s| s.carousel_index).unwrap_or(0);

            let max_tokens = state.as_ref().map(|s| s.max_tokens).unwrap_or(0);
            let used = state.as_ref().map(|s| s.used_tokens).unwrap_or(0);
            *state = Some(CapyState {
                commits: commit_data,
                pr: prs,
                workflow: workflows,
                carousel_index,
                used_tokens: used,
                max_tokens,
            });
        }

        info!("Successfully updated metrics. Next update in 60 seconds.");
        Timer::after_secs(60).await;
    }
}
#[derive(Debug)]
pub struct CommitData {
    pub total: u32,
    pub last_week: u32,
    pub last_month: u32,
}

fn parse_commits(all_time: &str, last_month: &str, last_week: &str) -> CommitData {
    let json = JSONValue::load(all_time);
    let x = json.get_key_value("total").unwrap().read_integer().unwrap();
    let json = JSONValue::load(last_month);
    let y = json.get_key_value("total").unwrap().read_integer().unwrap();
    let json = JSONValue::load(last_week);
    let z = json.get_key_value("total").unwrap().read_integer().unwrap();

    CommitData {
        total: x as u32,
        last_week: z as u32,
        last_month: y as u32,
    }
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
