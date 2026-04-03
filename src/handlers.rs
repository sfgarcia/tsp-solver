use axum::{extract::{Json, Query}, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use crate::tour::Tour;

// ── Bencineras ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BoundsQuery {
    pub south: f64,
    pub north: f64,
    pub west:  f64,
    pub east:  f64,
}

#[derive(Serialize)]
pub struct Bencinera {
    pub lat:       f64,
    pub lng:       f64,
    pub nombre:    String,
    pub direccion: String,
}

pub async fn bencineras(Query(b): Query<BoundsQuery>) -> impl IntoResponse {
    // Overpass API: amenity=fuel dentro del bounding box
    let query = format!(
        "[out:json][timeout:10];node[\"amenity\"=\"fuel\"]({},{},{},{});out;",
        b.south, b.west, b.north, b.east
    );

    let mirrors = [
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
        "https://overpass.private.coffee/api/interpreter",
    ];

    let client = reqwest::Client::new();
    let mut raw: Option<serde_json::Value> = None;

    for mirror in &mirrors {
        let result = client
            .get(*mirror)
            .query(&[("data", &query)])
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await;

        if let Ok(resp) = result {
            let text = resp.text().await.unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if json["elements"].is_array() {
                    raw = Some(json);
                    break;
                }
            }
        }
    }

    let raw = match raw {
        Some(v) => v,
        None => return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": "Overpass API no disponible, intenta de nuevo." })),
        ).into_response(),
    };

    let elements = match raw["elements"].as_array() {
        Some(a) => a,
        None => return (StatusCode::OK, Json(serde_json::json!([]))).into_response(),
    };

    let bencineras: Vec<Bencinera> = elements
        .iter()
        .filter_map(|e| {
            let lat = e["lat"].as_f64()?;
            let lng = e["lon"].as_f64()?;
            let tags = &e["tags"];
            let nombre = tags["name"].as_str()
                .or_else(|| tags["brand"].as_str())
                .unwrap_or("Bencinera")
                .to_string();
            let direccion = tags["addr:street"].as_str()
                .map(|s| {
                    let num = tags["addr:housenumber"].as_str().unwrap_or("");
                    if num.is_empty() { s.to_string() } else { format!("{} {}", s, num) }
                })
                .unwrap_or_default();
            Some(Bencinera { lat, lng, nombre, direccion })
        })
        .collect();

    (StatusCode::OK, Json(bencineras)).into_response()
}

#[derive(Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Deserialize)]
pub struct SolveRequest {
    pub coordinates: Vec<LatLng>,
}

#[derive(Serialize)]
pub struct SolveResponse {
    pub route: Vec<RoutePoint>,
    pub total_distance_km: f32,
}

#[derive(Serialize)]
pub struct RoutePoint {
    pub lat: f64,
    pub lng: f64,
}

pub async fn solve(Json(payload): Json<SolveRequest>) -> impl IntoResponse {
    if payload.coordinates.len() < 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "At least 3 coordinates are required." })),
        )
            .into_response();
    }

    let positions: Vec<(f32, f32)> = payload
        .coordinates
        .iter()
        .map(|c| (c.lat as f32, c.lng as f32))
        .collect();

    let result = tokio::task::spawn_blocking(move || {
        let mut tour = Tour::new(positions);
        tour.random_tour();
        tour.two_opt();
        tour.or_opt();
        tour.calculate_cost();
        tour
    })
    .await
    .unwrap();

    let route: Vec<RoutePoint> = result
        .route
        .iter()
        .map(|n| RoutePoint { lat: n.x as f64, lng: n.y as f64 })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "route": route,
            "total_distance_km": result.cost
        })),
    )
        .into_response()
}
