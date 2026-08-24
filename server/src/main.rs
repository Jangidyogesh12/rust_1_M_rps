use log::{error, info};
use shared::config::{
    database::{Database, DatabaseTrait},
    parameter,
    redis::{Redis, RedisTrait},
};
use std::process::exit;
use tokio::{
    net::TcpListener,
    signal::{self},
};

mod dto;
mod handler;
mod repository;
mod response;
mod routes;
mod service;
mod state;

#[tokio::main]
async fn main() {
    env_logger::init();

    if let Err(e) = parameter::init() {
        error!("Parameter init failed: {}", e);
        exit(1);
    }

    info!("Environment variable loaded");

    let redis = Redis::init().await.unwrap_or_else(|e| {
        error!("{}", e);
        exit(1)
    });

    let database = Database::init().await.unwrap_or_else(|e| {
        error!("{}", e);
        exit(1)
    });
    let pg_pool = database.get_pool().clone();

    let app_url = parameter::get("APP_URL").unwrap_or_else(|e| {
        error!("{}", e);
        exit(1);
    });

    let app_port = parameter::get("APP_PORT").unwrap_or_else(|e| {
        error!("{}", e);
        exit(1);
    });

    let host = format!("{}:{}", app_url, app_port);

    info!("Server starting at {}", host);

    let listener = TcpListener::bind(&host).await.unwrap_or_else(|e| {
        error!("Error binding address {}: {}", &host, e);
        exit(1);
    });

    let app = routes::root::routes(redis, pg_pool);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| error!("Server error: {}", e));
}

async fn shutdown_signal() {
    signal::ctrl_c().await.unwrap_or_else(|_| {
        error!("Failed to install CTRL+C handler");
        exit(1);
    })
}
