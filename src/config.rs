use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub streams: Vec<StreamConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub listen: String,
    pub ffmpeg_binary: String,
    pub supervisor_interval_ms: u64,

    /// HLS 切片存储根目录
    /// 建议配置为 /dev/shm/vtx-hls 以保护闪存寿命
    #[serde(default = "default_hls_root")]
    pub hls_root: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StreamConfig {
    pub name: String,
    pub source: String,
    pub output_args: Vec<String>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub idle_timeout: u64,

    /// 故障重试策略
    #[serde(default)]
    pub retry: RetryPolicy,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RetryPolicy {
    /// 最大重试次数 (0 表示无限重试)
    pub max_attempts: u32,
    /// 初始退避时间 (秒)
    pub initial_backoff_sec: u64,
    /// 最大退避时间 (秒)
    pub max_backoff_sec: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_backoff_sec: 2,
            max_backoff_sec: 60,
        }
    }
}

fn default_hls_root() -> String {
    "./static/hls".to_string()
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
