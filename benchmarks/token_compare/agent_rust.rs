use axum::{routing::{get, post}, Router, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct AskRequest {
    question: String,
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn ask(Json(body): Json<AskRequest>) -> String {
    let url = format!("https://api.example.com/ask?q={}", body.question);
    reqwest::get(&url).await.unwrap().text().await.unwrap()
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/ask", post(ask));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
