use crate::engine::Engine;
use crate::state::{AppState, StreamRecoveryState};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// 启动后台监控任务，定期检查流的状态并进行故障恢复和重启
///
/// # 任务流程：
/// - 每隔指定的时间间隔检查一次流的状态
/// - 检查流是否正常运行，如果流意外退出，记录并尝试重启
/// - 如果流超时空闲，则安排停止
/// - 在流崩溃后根据配置进行回退和重试
/// - 如果流自动重启配置为启用，尝试重启失败的流
pub async fn start_supervisor(state: Arc<AppState>, interval_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));

    loop {
        interval.tick().await; // 等待指定的时间间隔
        let now = Instant::now();
        let mut streams_to_kill = Vec::new(); // 用于存储待停止的流
        let mut streams_crashed = Vec::new(); // 用于存储崩溃的流

        // --- 阶段 1: 检查流状态 ---
        {
            let mut streams = state.active_streams.lock().unwrap();

            for (name, runtime) in streams.iter_mut() {
                match runtime.process.try_wait() {
                    Ok(Some(status)) => {
                        // 流异常退出，记录警告并加入崩溃列表
                        warn!("Stream [{}] exited unexpectedly with: {}", name, status);
                        streams_crashed.push(name.clone());
                        continue;
                    }
                    Ok(None) => {} // 流还在运行
                    Err(e) => error!("Process monitor error [{}]: {}", name, e), // 监控进程出错
                }

                // 检查流是否超时空闲
                if let Some(cfg) = state.config.streams.iter().find(|s| s.name == *name) {
                    if cfg.idle_timeout > 0 {
                        let idle_dur = now.duration_since(runtime.last_accessed);
                        if idle_dur.as_secs() > cfg.idle_timeout {
                            // 如果空闲超过配置的超时，安排停止流
                            info!(
                                "Stream [{}] idle for {}s. Scheduling stop.",
                                name,
                                idle_dur.as_secs()
                            );
                            streams_to_kill.push(name.clone());
                        }
                    }
                }
            }

            // 从活动流中移除崩溃的流
            for name in &streams_crashed {
                streams.remove(name);
            }
        }

        // --- 阶段 2: 执行停止流任务 ---
        for name in streams_to_kill {
            let _ = Engine::stop_stream(&state, &name).await;
        }

        // --- 阶段 3: 故障恢复 (Backoff) ---
        for name in streams_crashed {
            let mut recovery_map = state.recovery_states.lock().unwrap();
            let recovery = recovery_map
                .entry(name.clone())
                .or_insert(StreamRecoveryState {
                    crash_count: 0,
                    next_retry_at: None,
                });

            if let Some(cfg) = state.config.streams.iter().find(|s| s.name == name) {
                // 检查最大重试次数
                if cfg.retry.max_attempts > 0 && recovery.crash_count >= cfg.retry.max_attempts {
                    // 如果达到最大重试次数，则放弃重试
                    error!(
                        "Stream [{}] reached max retry attempts ({}). Giving up.",
                        name, cfg.retry.max_attempts
                    );
                    continue;
                }

                // 计算回退时间（基于指数退避算法）
                let backoff_sec = std::cmp::min(
                    cfg.retry.max_backoff_sec,
                    cfg.retry.initial_backoff_sec * 2u64.pow(recovery.crash_count),
                );

                recovery.crash_count += 1;
                recovery.next_retry_at = Some(now + Duration::from_secs(backoff_sec));

                // 记录警告，说明流崩溃并开始回退
                warn!(
                    "Stream [{}] crashed. Retry {}/{}. Backing off for {}s.",
                    name, recovery.crash_count, cfg.retry.max_attempts, backoff_sec
                );
            }
        }

        // --- 阶段 4: 尝试重启流任务 ---
        for cfg in &state.config.streams {
            if !cfg.auto_start {
                continue;
            } // 如果配置中不允许自动启动，跳过

            // 检查流是否已在运行
            let is_running = state.active_streams.lock().unwrap().contains_key(&cfg.name);
            if is_running {
                continue;
            }

            let mut should_start = true;
            {
                let recovery_map = state.recovery_states.lock().unwrap();
                if let Some(rec) = recovery_map.get(&cfg.name) {
                    // 如果已达到最大重试次数或还在冷却中，则不重启流
                    if rec.next_retry_at.is_none() && rec.crash_count > 0 {
                        should_start = false;
                    } else if let Some(next_retry) = rec.next_retry_at {
                        if now < next_retry {
                            should_start = false;
                        }
                    }
                }
            }

            if should_start {
                // 尝试重启流
                info!("Supervisor: Attempting to restart stream [{}]", cfg.name);
                if let Err(e) = Engine::start_stream(&state, &cfg.name).await {
                    // 重启失败，记录错误日志
                    error!("Restart failed [{}]: {}", cfg.name, e);
                }
            }
        }
    }
}
