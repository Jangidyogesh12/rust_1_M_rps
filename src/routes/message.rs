use axum::Router;

pub fn route() -> Router {
    let router = Router::new().route("/message", post());
    router
}
