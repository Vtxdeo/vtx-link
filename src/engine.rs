use crate::state::{AppState, StreamRuntime};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;
use tokio::process::Command;
use tracing::{error, info, warn};

pub struct Engine;

impl Engine {
    /// 启动指定名称的流任务
    ///
    /// # 副作用
    /// - 启动子进程
    /// - 清理并创建 HLS 输出目录
    ///
    /// # 错误处理
    /// - 内存不足时返回错误
    /// - 配置未找到时返回错误
    /// - FFmpeg 启动失败时返回错误
    pub async fn start_stream(state: &Arc<AppState>, name: &str) -> anyhow::Result<()> {
        // 1. 检查流任务是否已经在运行
        {
            let mut streams = state.active_streams.lock().unwrap();
            if let Some(running) = streams.get_mut(name) {
                // 如果流已在运行，则更新最后访问时间并直接返回
                running.last_accessed = Instant::now();
                return Ok(());
            }
        }

        // 2. 检查系统内存是否足够
        match sys_info::mem_info() {
            Ok(mem) => {
                // 如果系统可用内存小于 5MB，则返回内存不足错误
                if mem.avail < 5120 {
                    return Err(anyhow::anyhow!(
                        "Insufficient system memory ({} KB available)",
                        mem.avail
                    ));
                }
            }
            Err(e) => {
                // 如果无法获取内存信息，仅记录警告而不阻断流程
                warn!("Failed to check memory usage: {}", e);
            }
        }

        // 3. 查找配置文件中的流配置
        let cfg = state
            .config
            .streams
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Stream configuration not found"))?;

        // 4. 准备 HLS 输出目录，适配 RAMDisk
        let output_dir = std::path::Path::new(&state.config.server.hls_root).join(name);

        // 如果目录已存在，则删除并重新创建
        if output_dir.exists() {
            let _ = fs::remove_dir_all(&output_dir).await;
        }
        fs::create_dir_all(&output_dir).await?;

        info!("Starting stream [{}]. HLS Output: {:?}", name, output_dir);

        // 5. 构建 FFmpeg 命令并启动子进程
        let mut cmd = Command::new(&state.config.server.ffmpeg_binary);
        cmd.arg("-hide_banner").arg("-y");
        cmd.arg("-i").arg(&cfg.source);

        // 替换输出路径变量
        let dir_str = output_dir.to_string_lossy();
        for arg in &cfg.output_args {
            let final_arg = arg.replace("{output_dir}", &dir_str);
            cmd.arg(final_arg);
        }

        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        // 启动 FFmpeg 子进程
        let child = cmd.spawn().map_err(|e| {
            error!("Failed to spawn FFmpeg process: {}", e);
            e
        })?;

        // 6. 更新活动流状态
        {
            let mut streams = state.active_streams.lock().unwrap();
            streams.insert(
                name.to_string(),
                StreamRuntime {
                    process: child,
                    last_accessed: Instant::now(),
                    started_at: Instant::now(),
                },
            );
        }

        // 7. 重置恢复状态（如果有的话）
        {
            let mut recovery = state.recovery_states.lock().unwrap();
            if let Some(rec) = recovery.get_mut(name) {
                // 重置流的崩溃计数和下次重试时间
                rec.crash_count = 0;
                rec.next_retry_at = None;
            }
        }

        Ok(())
    }

    /// 停止指定名称的流任务
    ///
    /// # 错误处理
    /// - 若流未找到，则返回空结果
    pub async fn stop_stream(state: &Arc<AppState>, name: &str) -> anyhow::Result<()> {
        let running_stream = {
            let mut streams = state.active_streams.lock().unwrap();
            streams.remove(name)
        };

        // 如果流正在运行，则尝试停止进程
        if let Some(mut running) = running_stream {
            let _ = running.process.kill().await;
            info!("Stream [{}] stopped.", name);
        }

        Ok(())
    }
}
