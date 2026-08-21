use axum::{Json, Router, routing::get};
use serde_json::json;

pub fn route() -> Router {
    Router::new().route(
        "/test",
        get(|| async {
            return Json(json!({"message":"Hello How Are you"}));
        }),
    )
}
