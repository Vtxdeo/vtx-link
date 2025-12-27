mod config;
mod engine;
mod state;
mod supervisor;
mod web;

use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use config::AppConfig;
use state::AppState;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::info;

/// VTX Link - Edge Media Gateway
/// 解析命令行参数，初始化服务，加载配置文件，并启动HTTP服务及后台监控
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "vtx-link.yaml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志系统，设置格式
    tracing_subscriber::fmt::init();

    // 解析命令行参数，获取配置文件路径
    let args = Args::parse();

    // 加载配置文件
    let config = AppConfig::load(&args.config)?;
    info!("VTX Link initialized. HLS Root: {}", config.server.hls_root);

    // 初始化全局状态，包含配置信息和活动流状态
    let state = Arc::new(AppState {
        config: config.clone(),
        active_streams: Mutex::new(HashMap::new()),
        recovery_states: Mutex::new(HashMap::new()),
    });

    // 启动后台监控程序
    let supervisor_interval = config.server.supervisor_interval_ms;
    tokio::spawn(supervisor::start_supervisor(
        state.clone(),
        supervisor_interval,
    ));

    // 注册HTTP路由
    let app = Router::new()
        .route("/", get(web::admin::index_handler)) // 首页
        .route("/sys/status", get(web::admin::sys_status)) // 系统状态
        .route("/streams", get(web::admin::list_streams)) // 获取流列表
        .route("/streams/:name/start", post(web::admin::handle_start)) // 启动流
        .route("/streams/:name/stop", post(web::admin::handle_stop)) // 停止流
        .route(
            "/hls/:stream_name/:file_name",
            get(web::hls::serve_hls_file), // 获取HLS文件
        )
        .with_state(state.clone());

    // 启动HTTP服务，监听指定的地址和端口
    info!("Listening on {}", config.server.listen);
    let listener = tokio::net::TcpListener::bind(&config.server.listen).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
