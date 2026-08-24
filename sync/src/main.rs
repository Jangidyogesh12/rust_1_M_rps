use std::process::exit;

use log::error;
use shared::{
    config::{
        database::{Database, DatabaseTrait},
        parameter,
        redis::{Redis, RedisTrait},
    },
};

mod consumer;
use consumer::message_consumer::MessageConsumer;

#[tokio::main]
async fn main() {
    env_logger::init();

    parameter::init().unwrap_or_else(|e| {
        error!("Parameter init failed: {}", e);
        exit(1);
    });

    let consumer_name = parameter::get("CONSUMER_NAME").unwrap_or_else(|e| {
        error!("{}", e);
        exit(1);
    });

    let redis = Redis::init().await.unwrap_or_else(|e| {
        error!("{}", e);
        exit(1);
    });

    let database = Database::init().await.unwrap_or_else(|e| {
        error!("{}", e);
        exit(1);
    });

    let consumer = MessageConsumer::new(
        redis.get_connection(),
        database.get_pool().clone(),
        consumer_name,
    );

    consumer.run().await;
}
