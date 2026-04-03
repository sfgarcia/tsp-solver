pub mod handlers;
pub mod tour;

#[tokio::main]
async fn main() {
    let html = include_str!("../static/index.html");

    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(move || async move { axum::response::Html(html) }),
        )
        .route("/solve", axum::routing::post(handlers::solve))
        .layer(tower_http::cors::CorsLayer::permissive());

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Listening on http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
