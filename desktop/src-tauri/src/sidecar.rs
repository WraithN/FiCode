use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::AppHandle;
use tokio::time::sleep;

pub struct SidecarManager {
    process: Option<Child>,
    pub port: u16,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: None,
            port: 4040,
        }
    }

    pub fn start(&mut self, _app_handle: &AppHandle) -> Result<(), String> {
        let binary_path = self.locate_binary()?;

        let mut cmd = Command::new(&binary_path);
        cmd.arg("--server")
            .arg("--port")
            .arg(self.port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().map_err(|e| format!("Failed to spawn sidecar: {}", e))?;
        self.process = Some(child);
        Ok(())
    }

    fn locate_binary(&self) -> Result<std::path::PathBuf, String> {
        let dev_path = std::path::PathBuf::from("../target/debug/fi-code");
        if dev_path.exists() {
            return Ok(dev_path);
        }
        Err("Sidecar binary not found. Please build fi-code first: cargo build".to_string())
    }

    pub async fn wait_ready(&self, timeout_secs: u64) -> Result<(), String> {
        let client = reqwest::Client::new();
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            let url = format!("http://127.0.0.1:{}/rpc", self.port);
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "get_status",
                "id": 1
            });

            if let Ok(resp) = client.post(&url).json(&body).timeout(Duration::from_secs(2)).send().await {
                if resp.status().is_success() {
                    return Ok(());
                }
            }

            sleep(Duration::from_millis(500)).await;
        }

        Err(format!("Sidecar not ready after {}s", timeout_secs))
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
        }
    }

    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop();
    }
}
