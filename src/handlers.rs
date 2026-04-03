use axum::{extract::{Json, Query, State}, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use crate::tour::Tour;

// ── Constants ──────────────────────────────────────────────────────────────────

const MAX_NODES: usize = 200;
const SOLVER_TIMEOUT_SECS: u64 = 30;
const BENCINERA_TTL_SECS: u64 = 86400;
pub const CACHE_FILE: &str = "bencineras_cache.json";

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn is_fresh(timestamp: u64) -> bool {
    now_unix().saturating_sub(timestamp) < BENCINERA_TTL_SECS
}

// ── Validation ────────────────────────────────────────────────────────────────

fn is_valid_coord(lat: f64, lng: f64) -> bool {
    lat.is_finite() && lng.is_finite() && lat >= -90.0 && lat <= 90.0 && lng >= -180.0 && lng <= 180.0
}

// ── Cache persistence ─────────────────────────────────────────────────────────

pub type CacheMap = HashMap<String, (Vec<Bencinera>, u64)>;

pub async fn load_cache_from_disk() -> CacheMap {
    match tokio::fs::read_to_string(CACHE_FILE).await {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => CacheMap::new(),
    }
}

async fn save_cache_to_disk(snapshot: CacheMap) {
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = tokio::fs::write(CACHE_FILE, json).await;
    }
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppState {
    pub cache: RwLock<CacheMap>,
    pub client: reqwest::Client,
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
}

pub async fn bencineras(
    State(state): State<SharedState>,
    Query(b): Query<BoundsQuery>,
) -> impl IntoResponse {
    // Validate bounds
    if !is_valid_coord(b.south, b.west) || !is_valid_coord(b.north, b.east) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Invalid bounds." }))).into_response();
    }

    let key = format!("{:.2}:{:.2}:{:.2}:{:.2}", b.south, b.north, b.west, b.east);
    {
        let guard = state.cache.read().await;
        if let Some((data, ts)) = guard.get(&key) {
            if is_fresh(*ts) {
                return (StatusCode::OK, Json(data.clone())).into_response();
            }
        }
    }

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

    let mut raw: Option<serde_json::Value> = None;

    for mirror in &mirrors {
        let result = state.client
            .get(*mirror)
            .query(&[("data", &query)])
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
            Json(serde_json::json!({ "error": "Overpass API unavailable." })),
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

    {
        let mut guard = state.cache.write().await;
        // Evict stale entries before inserting new one
        guard.retain(|_, (_, ts)| is_fresh(*ts));
        guard.insert(key, (bencineras.clone(), now_unix()));

        // Spawn background task to persist cache to disk
        let snapshot = guard.clone();
        tokio::spawn(async move {
            save_cache_to_disk(snapshot).await;
        });
    }
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

    if payload.coordinates.len() > MAX_NODES {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Maximum {} coordinates allowed.", MAX_NODES) })),
        )
            .into_response();
    }

    // Validate all coordinates
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
            tour.random_tour();
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
