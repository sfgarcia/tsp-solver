pub mod handlers;
pub mod tour;

use axum::response::Response;
use axum::http::{header, StatusCode};
use axum::body::Body;
use crate::handlers::AppState;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const TOKEN_REFRESH_SECS: u64 = 50 * 60; // 50 min (token expires in 60 min)

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

    let cne_email    = std::env::var("CNE_EMAIL").expect("CNE_EMAIL must be set");
    let cne_password = std::env::var("CNE_PASSWORD").expect("CNE_PASSWORD must be set");

    println!("Logging in to CNE API...");
    let token = handlers::login_cne(&client, &cne_email, &cne_password)
        .await
        .expect("CNE login failed — check CNE_EMAIL and CNE_PASSWORD");

    println!("Loading CNE stations...");
    let stations = handlers::fetch_cne_stations(&client, &token).await;
    println!("Loaded {} CNE stations", stations.len());

    let state = Arc::new(AppState {
        client,
        cne_stations: RwLock::new(stations),
    });

    // Background task: re-login and refresh stations every 50 minutes
    let state_bg   = Arc::clone(&state);
    let email_bg   = cne_email.clone();
    let password_bg = cne_password.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(TOKEN_REFRESH_SECS)).await;
            match handlers::login_cne(&state_bg.client, &email_bg, &password_bg).await {
                Some(new_token) => {
                    let stations = handlers::fetch_cne_stations(&state_bg.client, &new_token).await;
                    println!("Refreshed {} CNE stations", stations.len());
                    *state_bg.cne_stations.write().await = stations;
                }
                None => eprintln!("CNE token refresh failed, retrying next cycle"),
            }
        }
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
        .route("/route-geometry", axum::routing::post(handlers::route_geometry))
        .route("/status", axum::routing::get(handlers::status))
        .route("/debug-cne", axum::routing::get(handlers::debug_cne))
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
