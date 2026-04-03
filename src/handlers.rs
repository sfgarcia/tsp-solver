use axum::{extract::{Json, Query, State}, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use crate::tour::Tour;

// ── Constants ──────────────────────────────────────────────────────────────────

const MAX_NODES: usize = 200;
const SOLVER_TIMEOUT_SECS: u64 = 30;

// ── Validation ────────────────────────────────────────────────────────────────

fn is_valid_coord(lat: f64, lng: f64) -> bool {
    lat.is_finite() && lng.is_finite() && lat >= -90.0 && lat <= 90.0 && lng >= -180.0 && lng <= 180.0
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppState {
    pub client: reqwest::Client,
    pub cne_stations: RwLock<Vec<CneStation>>,
}

pub type SharedState = Arc<AppState>;

// ── Bencineras ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BoundsQuery {
    pub south: f64,
    pub north: f64,
    pub west:  f64,
    pub east:  f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Bencinera {
    pub lat:       f64,
    pub lng:       f64,
    pub nombre:    String,
    pub direccion: String,
    pub precio_93: Option<String>,
    pub precio_95: Option<String>,
    pub precio_97: Option<String>,
}

// ── CNE Stations ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CneStation {
    pub lat:       f64,
    pub lng:       f64,
    pub nombre:    String,
    pub direccion: String,
    pub precio_93: Option<String>,
    pub precio_95: Option<String>,
    pub precio_97: Option<String>,
}

fn parse_cne_station(s: &serde_json::Value) -> Option<CneStation> {
    let ubicacion = &s["ubicacion"];
    let lat = ubicacion["latitud"].as_str()?.parse::<f64>().ok()?;
    let lng = ubicacion["longitud"].as_str()?.parse::<f64>().ok()?;
    let nombre = s["razon_social"].as_str().unwrap_or("Bencinera").to_string();
    let direccion = ubicacion["direccion"].as_str().unwrap_or("").trim().to_string();
    let precios = &s["precios"];

    Some(CneStation {
        lat,
        lng,
        nombre,
        direccion,
        precio_93: precios["93"]["precio"].as_str().map(|p| p.to_string()),
        precio_95: precios["95"]["precio"].as_str().map(|p| p.to_string()),
        precio_97: precios["97"]["precio"].as_str().map(|p| p.to_string()),
    })
}

pub async fn fetch_cne_stations(client: &reqwest::Client, token: &str) -> Vec<CneStation> {
    let resp = client
        .get("https://api.cne.cl/api/v4/estaciones")
        .bearer_auth(token)
        .send()
        .await;

    match resp {
        Ok(r) => match r.json::<serde_json::Value>().await {
            Ok(json) => json.as_array()
                .map(|arr| arr.iter().filter_map(parse_cne_station).collect())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn bencineras(
    State(state): State<SharedState>,
    Query(b): Query<BoundsQuery>,
) -> impl IntoResponse {
    if !is_valid_coord(b.south, b.west) || !is_valid_coord(b.north, b.east) {
        return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid bounds." }))).into_response();
    }

    let cne = state.cne_stations.read().await;
    let stations: Vec<Bencinera> = cne.iter()
        .filter(|s| {
            s.lat >= b.south && s.lat <= b.north
                && s.lng >= b.west && s.lng <= b.east
        })
        .map(|s| Bencinera {
            lat:       s.lat,
            lng:       s.lng,
            nombre:    s.nombre.clone(),
            direccion: s.direccion.clone(),
            precio_93: s.precio_93.clone(),
            precio_95: s.precio_95.clone(),
            precio_97: s.precio_97.clone(),
        })
        .collect();

    (StatusCode::OK, Json(stations)).into_response()
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

    if payload.coordinates.len() > MAX_NODES {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Maximum {} coordinates allowed.", MAX_NODES) })),
        )
            .into_response();
    }

    if payload.coordinates.iter().any(|c| !is_valid_coord(c.lat, c.lng)) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid coordinates." })),
        )
            .into_response();
    }

    let positions: Vec<(f32, f32)> = payload
        .coordinates
        .iter()
        .map(|c| (c.lat as f32, c.lng as f32))
        .collect();

    let result = tokio::time::timeout(
        Duration::from_secs(SOLVER_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            let mut tour = Tour::new(positions);
            tour.nearest_neighbour_tour();
            tour.two_opt();
            tour.or_opt();
            tour.calculate_cost();
            tour
        }),
    )
    .await;

    let tour = match result {
        Ok(Ok(t)) => t,
        Ok(Err(_)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Solver failed." })),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::REQUEST_TIMEOUT,
                Json(serde_json::json!({ "error": "Solver timed out." })),
            )
                .into_response();
        }
    };

    let route: Vec<RoutePoint> = tour
        .route
        .iter()
        .map(|n| RoutePoint { lat: n.x as f64, lng: n.y as f64 })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "route": route,
            "total_distance_km": tour.cost
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_coord_valid_cases() {
        assert!(is_valid_coord(0.0, 0.0));
        assert!(is_valid_coord(-90.0, -180.0));
        assert!(is_valid_coord(90.0, 180.0));
        assert!(is_valid_coord(-33.87, -70.72)); // Santiago
        assert!(is_valid_coord(51.5, -0.1));     // London
    }

    #[test]
    fn is_valid_coord_out_of_range_latitude() {
        assert!(!is_valid_coord(91.0, 0.0));
        assert!(!is_valid_coord(-91.0, 0.0));
    }

    #[test]
    fn is_valid_coord_out_of_range_longitude() {
        assert!(!is_valid_coord(0.0, 181.0));
        assert!(!is_valid_coord(0.0, -181.0));
    }

    #[test]
    fn is_valid_coord_nan() {
        assert!(!is_valid_coord(f64::NAN, 0.0));
        assert!(!is_valid_coord(0.0, f64::NAN));
    }

    #[test]
    fn is_valid_coord_infinity() {
        assert!(!is_valid_coord(f64::INFINITY, 0.0));
        assert!(!is_valid_coord(0.0, f64::NEG_INFINITY));
    }

    #[test]
    fn is_valid_coord_boundaries() {
        assert!(is_valid_coord(-90.0, -180.0));
        assert!(is_valid_coord(90.0, 180.0));
        assert!(!is_valid_coord(-90.1, 0.0));
        assert!(!is_valid_coord(90.1, 0.0));
    }
}
