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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
