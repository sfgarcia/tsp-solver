pub mod handlers;
pub mod tour;

use axum::response::Response;
use axum::http::{header, StatusCode};
use axum::body::Body;
use crate::handlers::AppState;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let html     = include_str!("../static/index.html");
    let manifest = include_str!("../static/manifest.json");
    let sw       = include_str!("../static/sw.js");
    let icon     = include_str!("../static/icon.svg");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .expect("failed to build reqwest client");

    let cne_token = std::env::var("CNE_TOKEN").expect("CNE_TOKEN must be set");
    println!("Loading CNE stations...");
    let stations = handlers::fetch_cne_stations(&client, &cne_token).await;
    println!("Loaded {} CNE stations", stations.len());

    let state = Arc::new(AppState {
        client,
        cne_stations: RwLock::new(stations),
    });

    let app = axum::Router::new()
        .route("/", axum::routing::get(move || async move {
            axum::response::Html(html)
        }))
        .route("/manifest.json", axum::routing::get(move || async move {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/manifest+json")
                .body(Body::from(manifest))
                .expect("failed to build manifest response")
        }))
        .route("/sw.js", axum::routing::get(move || async move {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/javascript")
                .body(Body::from(sw))
                .expect("failed to build sw response")
        }))
        .route("/icon.svg", axum::routing::get(move || async move {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/svg+xml")
                .body(Body::from(icon))
                .expect("failed to build icon response")
        }))
        .route("/solve", axum::routing::post(handlers::solve))
        .route("/bencineras", axum::routing::get(handlers::bencineras))
        .with_state(state)
        .layer(tower_http::cors::CorsLayer::permissive());

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await
        .expect("failed to bind TCP listener");
    println!("Listening on http://{}", addr);
    axum::serve(listener, app).await
        .expect("server error");
}
