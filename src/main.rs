use crate::config::{
    database::{Database, DatabaseTrait},
    parameter,
};
use axum;
use log::{error, info};
use std::process::exit;
use tokio::{
    net::TcpListener,
    signal::{self},
};

mod config;
mod dto;
mod entity;
mod error;
mod handler;
mod repository;
mod response;
mod routes;
mod service;
mod state;

#[tokio::main]
async fn main() {
    let db_url = "postgres://dbuser:mysecretpassword@localhost:5432/bookstore";
    let db_conn = Database::init().await.unwrap_or_else(|e| {
        error!("Failed to initialize database: {}", e);
        exit(1);
    });

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

    let listener = TcpListener::bind(host).await.unwrap_or_else(|e| {
        error!("Error binding address {}: {}", host, e);
        exit(1);
    });

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
