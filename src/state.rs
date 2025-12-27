use crate::config::AppConfig;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::process::Child;

/// 运行时的流实例状态
pub struct StreamRuntime {
    /// FFmpeg 子进程句柄
    pub process: Child,
    /// 最后一次活跃时间 (用于空闲回收)
    pub last_accessed: Instant,
    /// 进程启动时间 (用于计算运行时长)
    pub started_at: Instant,
}

/// 故障恢复状态
pub struct StreamRecoveryState {
    /// 连续崩溃次数
    pub crash_count: u32,
    /// 下次允许尝试重启的最早时间点
    pub next_retry_at: Option<Instant>,
}

/// 全局应用上下文
pub struct AppState {
    pub config: AppConfig,
    /// 活跃流表 (Stream Name -> Runtime)
    pub active_streams: Mutex<HashMap<String, StreamRuntime>>,
    /// 恢复状态表 (Stream Name -> Recovery State)
    pub recovery_states: Mutex<HashMap<String, StreamRecoveryState>>,
}

pub type SharedState = Arc<AppState>;