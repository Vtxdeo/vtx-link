use crate::state::SharedState;
use crate::engine::Engine;
use axum::{
    extract::{Path, State},
    Json,
};
use std::time::Instant;

/// 提供内嵌的管理后台页面
/// 该处理函数返回嵌入的 HTML 页面，用于管理界面
pub async fn index_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../../static/index.html"))
}

/// 获取系统状态 API
/// 该处理函数返回系统的内存和负载信息，作为 JSON 响应
pub async fn sys_status() -> Json<serde_json::Value> {
    // 获取内存信息，默认值为 0
    let mem = sys_info::mem_info().map(|m| (m.total, m.avail)).unwrap_or((0, 0));
    // 获取负载信息，默认值为 0.0
    let load = sys_info::loadavg().map(|l| l.one).unwrap_or(0.0);

    // 返回系统的内存和负载状态
    Json(serde_json::json!({
        "mem_total": mem.0 / 1024, // 转换为MB
        "mem_avail": mem.1 / 1024, // 转换为MB
        "load_avg": load,
    }))
}

/// 获取流列表 API
/// 返回所有流的状态信息，包括每个流的运行时长和闲置时间
pub async fn list_streams(State(state): State<SharedState>) -> Json<serde_json::Value> {
    // 获取当前活跃流和恢复状态
    let streams_map = state.active_streams.lock().unwrap();
    let recovery_map = state.recovery_states.lock().unwrap();
    let now = Instant::now();

    // 遍历配置文件中的流，生成每个流的状态信息
    let result: Vec<_> = state.config.streams.iter().map(|cfg| {
        // 获取流的状态、闲置时间和运行时长
        let (status, idle, uptime) = if let Some(running) = streams_map.get(&cfg.name) {
            let idle_sec = now.duration_since(running.last_accessed).as_secs();
            let uptime_sec = now.duration_since(running.started_at).as_secs();
            ("running", idle_sec, uptime_sec)
        } else {
            ("stopped", 0, 0)
        };

        // 获取流的崩溃次数（如果有）
        let crash_count = recovery_map.get(&cfg.name).map(|r| r.crash_count).unwrap_or(0);

        // 返回每个流的状态信息
        serde_json::json!({
            "name": cfg.name,
            "source": cfg.source,
            "status": status,
            "idle_seconds": idle,
            "uptime_seconds": uptime,
            "config_idle_timeout": cfg.idle_timeout,
            "crash_count": crash_count
        })
    }).collect();

    // 返回所有流的信息
    Json(serde_json::json!({ "streams": result }))
}

/// 手动启动流 API
/// 启动指定名称的流，并返回操作结果信息
pub async fn handle_start(
    State(state): State<SharedState>,
    Path(name): Path<String>
) -> String {
    match Engine::start_stream(&state, &name).await {
        Ok(_) => format!("Stream [{}] is active (started or refreshed)", name),
        Err(e) => format!("Error: {}", e),
    }
}

/// 手动停止流 API
/// 停止指定名称的流，并返回操作结果信息
pub async fn handle_stop(
    State(state): State<SharedState>,
    Path(name): Path<String>
) -> String {
    match Engine::stop_stream(&state, &name).await {
        Ok(_) => format!("Stream [{}] stopped", name),
        Err(e) => format!("Error: {}", e),
    }
}
