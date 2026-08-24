use std::env;

use dotenv::dotenv;
use log::{error, info};

pub fn init() -> Result<(), String> {
    match dotenv() {
        Ok(_) => {
            info!("Loading .env file");
            Ok(())
        }
        Err(e) => {
            error!("Failed to load .env file: {}", e);
            Err(e.to_string())
        }
    }
}

pub fn get(parameter: &str) -> Result<String, String> {
    match env::var(parameter) {
        Ok(value) => Ok(value),
        Err(_) => {
            let msg = format!("Environment Variable {} is not set", parameter);
            error!("{}", msg);
            Err(msg)
        }
    }
}
