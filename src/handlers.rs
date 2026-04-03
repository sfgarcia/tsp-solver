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

pub type CommuneStore = HashMap<String, (Vec<Bencinera>, u64)>;

pub async fn load_cache_from_disk() -> CommuneStore {
    match tokio::fs::read_to_string(CACHE_FILE).await {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => CommuneStore::new(),
    }
}

async fn save_cache_to_disk(snapshot: CommuneStore) {
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = tokio::fs::write(CACHE_FILE, json).await;
    }
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppState {
    pub communes: RwLock<CommuneStore>,
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

// ── Helpers for Overpass parsing ───────────────────────────────────────────────

fn parse_bencinera(e: &serde_json::Value) -> Option<Bencinera> {
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
}

// ── Overpass API functions ──────────────────────────────────────────────────────

async fn fetch_communes_in_bbox(
    client: &reqwest::Client,
    south: f64, north: f64, west: f64, east: f64,
) -> Vec<String> {
    let query = format!(
        "[out:json][timeout:10];\
         rel[\"boundary\"=\"administrative\"][\"admin_level\"=\"8\"]({},{},{},{});\
         out center;",
        south, west, north, east
    );

    let mirrors = [
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
        "https://overpass.private.coffee/api/interpreter",
    ];

    for mirror in &mirrors {
        let Ok(resp) = client.get(*mirror).query(&[("data", &query)]).send().await else {
            continue;
        };
        let Ok(text) = resp.text().await else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };

        if let Some(elements) = json["elements"].as_array() {
            return elements
                .iter()
                .filter_map(|e| e["tags"]["name"].as_str().map(String::from))
                .collect();
        }
    }

    Vec::new()
}

async fn fetch_bencineras_for_commune(
    client: &reqwest::Client,
    commune_name: &str,
) -> Option<Vec<Bencinera>> {
    let escaped_name = commune_name.replace('"', "\\\"");
    let query = format!(
        "[out:json][timeout:30];\
         area[\"name\"=\"{}\"][\"boundary\"=\"administrative\"][\"admin_level\"=\"8\"]->.a;\
         node[\"amenity\"=\"fuel\"](area.a);\
         out;",
        escaped_name
    );

    let mirrors = [
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
        "https://overpass.private.coffee/api/interpreter",
    ];

    for mirror in &mirrors {
        let Ok(resp) = client.get(*mirror).query(&[("data", &query)]).send().await else {
            continue;
        };
        let Ok(text) = resp.text().await else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };

        if let Some(elements) = json["elements"].as_array() {
            let bencineras: Vec<Bencinera> = elements
                .iter()
                .filter_map(parse_bencinera)
                .collect();
            return Some(bencineras);
        }
    }

    None
}

async fn fetch_bencineras_bbox_direct(
    client: &reqwest::Client,
    south: f64, north: f64, west: f64, east: f64,
) -> Option<Vec<Bencinera>> {
    let query = format!(
        "[out:json][timeout:10];node[\"amenity\"=\"fuel\"]({},{},{},{});out;",
        south, west, north, east
    );

    let mirrors = [
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
        "https://overpass.private.coffee/api/interpreter",
    ];

    for mirror in &mirrors {
        let Ok(resp) = client.get(*mirror).query(&[("data", &query)]).send().await else {
            continue;
        };
        let Ok(text) = resp.text().await else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };

        if let Some(elements) = json["elements"].as_array() {
            let bencineras: Vec<Bencinera> = elements
                .iter()
                .filter_map(parse_bencinera)
                .collect();
            return Some(bencineras);
        }
    }

    None
}

pub async fn bencineras(
    State(state): State<SharedState>,
    Query(b): Query<BoundsQuery>,
) -> impl IntoResponse {
    // ── 1. Validate bounds ──────────────────────────────────────────────────
    if !is_valid_coord(b.south, b.west) || !is_valid_coord(b.north, b.east) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Invalid bounds." }))).into_response();
    }

    // ── 2. Discover communes in bbox ────────────────────────────────────────
    let communes_in_bbox = fetch_communes_in_bbox(
        &state.client, b.south, b.north, b.west, b.east
    ).await;

    // ── 3. Fallback if no communes ──────────────────────────────────────────
    if communes_in_bbox.is_empty() {
        return match fetch_bencineras_bbox_direct(
            &state.client, b.south, b.north, b.west, b.east
        ).await {
            Some(bencineras) => (StatusCode::OK, Json(bencineras)).into_response(),
            None => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": "Overpass API unavailable." }))).into_response(),
        };
    }

    // ── 4. Classify communes: cache hit vs need fetch ───────────────────────
    let mut all_bencineras: Vec<Bencinera> = Vec::new();
    let mut communes_to_fetch: Vec<String> = Vec::new();

    {
        let guard = state.communes.read().await;
        for commune in &communes_in_bbox {
            match guard.get(commune) {
                Some((data, ts)) if is_fresh(*ts) => {
                    all_bencineras.extend_from_slice(data);
                }
                _ => {
                    communes_to_fetch.push(commune.clone());
                }
            }
        }
    }

    // ── 5. Fetch missing communes (parallelized) ────────────────────────────
    let mut newly_fetched: Vec<(String, Vec<Bencinera>)> = Vec::new();

    if !communes_to_fetch.is_empty() {
        let handles: Vec<_> = communes_to_fetch
            .iter()
            .map(|commune| {
                let client = state.client.clone();
                let commune = commune.clone();
                tokio::spawn(async move {
                    let result = fetch_bencineras_for_commune(&client, &commune).await;
                    (commune, result)
                })
            })
            .collect();

        for handle in handles {
            if let Ok((commune, Some(bencineras))) = handle.await {
                all_bencineras.extend_from_slice(&bencineras);
                newly_fetched.push((commune, bencineras));
            }
        }
    }

    // ── 6. Persist new entries to cache ─────────────────────────────────────
    if !newly_fetched.is_empty() {
        let mut guard = state.communes.write().await;
        let ts = now_unix();
        guard.retain(|_, (_, t)| is_fresh(*t));
        for (commune, bencineras) in newly_fetched {
            guard.insert(commune, (bencineras, ts));
        }
        let snapshot = guard.clone();
        tokio::spawn(async move {
            save_cache_to_disk(snapshot).await;
        });
    }

    // ── 7. Dedup by position ────────────────────────────────────────────────
    all_bencineras.sort_by(|a, b| {
        a.lat.partial_cmp(&b.lat).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.lng.partial_cmp(&b.lng).unwrap_or(std::cmp::Ordering::Equal))
    });
    all_bencineras.dedup_by(|a, b| a.lat == b.lat && a.lng == b.lng);

    // ── 8. Filter to bbox ───────────────────────────────────────────────────
    all_bencineras.retain(|item| {
        item.lat >= b.south && item.lat <= b.north
            && item.lng >= b.west && item.lng <= b.east
    });

    (StatusCode::OK, Json(all_bencineras)).into_response()
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
