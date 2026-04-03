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

/// Tile size in degrees (~11 km). Each tile covers [lat, lat+SIZE) × [lng, lng+SIZE).
const TILE_SIZE: f64 = 0.1;

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

// ── Tile helpers ──────────────────────────────────────────────────────────────

/// Round down to the nearest tile boundary.
fn tile_origin(v: f64) -> f64 {
    (v / TILE_SIZE).floor() * TILE_SIZE
}

/// Stable string key for a tile: "{south:.1}:{west:.1}"
fn tile_key(south: f64, west: f64) -> String {
    format!("{:.1}:{:.1}", south, west)
}

/// Return all tile (south, west) origins that overlap the given bbox.
fn tiles_in_bbox(south: f64, north: f64, west: f64, east: f64) -> Vec<(f64, f64)> {
    let mut tiles = Vec::new();
    let mut lat = tile_origin(south);
    while lat < north {
        let mut lng = tile_origin(west);
        while lng < east {
            tiles.push((lat, lng));
            lng = (lng * 10.0 + 1.0).round() / 10.0; // advance by exactly TILE_SIZE
        }
        lat = (lat * 10.0 + 1.0).round() / 10.0;
    }
    tiles
}

// ── Cache persistence ─────────────────────────────────────────────────────────
//
// Key format: "{south:.1}:{west:.1}"  (exactly one colon)
// Legacy bbox keys (e.g. "-33.50:-33.40:-70.70:-70.55") have 3 colons and are dropped.
// Legacy commune keys (no colon) are also dropped.

pub type TileCache = HashMap<String, (Vec<Bencinera>, u64)>;

pub async fn load_cache_from_disk() -> TileCache {
    match tokio::fs::read_to_string(CACHE_FILE).await {
        Ok(s) => {
            let raw: TileCache = serde_json::from_str(&s).unwrap_or_default();
            raw.into_iter()
                .filter(|(k, _)| k.matches(':').count() == 1)
                .collect()
        }
        Err(_) => TileCache::new(),
    }
}

async fn save_cache_to_disk(snapshot: TileCache) {
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = tokio::fs::write(CACHE_FILE, json).await;
    }
}

// ── Application State ──────────────────────────────────────────────────────────

pub struct AppState {
    pub tiles: RwLock<TileCache>,
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

// ── CNE Stations (for price enrichment) ────────────────────────────────────

#[derive(Clone)]
pub struct CneStation {
    pub lat: f64,
    pub lng: f64,
    pub precio_93: Option<String>,
    pub precio_95: Option<String>,
    pub precio_97: Option<String>,
}

// ── Overpass API ──────────────────────────────────────────────────────────────

const MIRRORS: [&str; 3] = [
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass-api.de/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
];

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
    let precio_93 = tags["fuel:93"].as_str().map(|s| s.to_string());
    let precio_95 = tags["fuel:95"].as_str().map(|s| s.to_string());
    let precio_97 = tags["fuel:97"].as_str().map(|s| s.to_string());
    Some(Bencinera { lat, lng, nombre, direccion, precio_93, precio_95, precio_97 })
}

/// Fetch all amenity=fuel nodes inside [south, north] × [west, east].
/// Returns None only if every mirror is unreachable.
async fn fetch_tile(
    client: &reqwest::Client,
    south: f64, north: f64, west: f64, east: f64,
) -> Option<Vec<Bencinera>> {
    let query = format!(
        "[out:json][timeout:15];node[\"amenity\"=\"fuel\"]({},{},{},{});out;",
        south, west, north, east
    );

    for mirror in &MIRRORS {
        let Ok(resp) = client.get(*mirror).query(&[("data", &query)]).send().await else {
            continue;
        };
        let Ok(text) = resp.text().await else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };

        if let Some(elements) = json["elements"].as_array() {
            return Some(elements.iter().filter_map(parse_bencinera).collect());
        }
    }
    None
}

/// Fetch fuel prices from CNE API for Metropolitana region.
/// Returns list of stations with lat/lng and current fuel prices.
pub async fn fetch_cne_stations(client: &reqwest::Client, token: &str) -> Vec<CneStation> {
    let url = format!("http://api.cne.cl/v3/combustibles/vehicular/estaciones?token={}", token);

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => {
                if let Some(stations) = json.as_array() {
                    stations.iter().filter_map(|s| {
                        let lat = s["latitud"].as_f64()?;
                        let lng = s["longitud"].as_f64()?;
                        let precios = &s["precio_por_combustible"];

                        Some(CneStation {
                            lat,
                            lng,
                            precio_93: precios["gasolina_93"].as_i64().map(|p| p.to_string()),
                            precio_95: precios["gasolina_95"].as_i64().map(|p| p.to_string()),
                            precio_97: precios["gasolina_97"].as_i64().map(|p| p.to_string()),
                        })
                    }).collect()
                } else {
                    Vec::new()
                }
            }
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

/// Find nearest CNE station within max_km distance.
fn nearest_cne_station(stations: &[CneStation], lat: f64, lng: f64, max_km: f64) -> Option<&CneStation> {
    use crate::tour::haversine_km;

    let mut best: Option<(&CneStation, f64)> = None;
    for station in stations {
        let dist = haversine_km(lat, lng, station.lat, station.lng);
        if dist <= max_km {
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((station, dist));
            }
        }
    }
    best.map(|(s, _)| s)
}

pub async fn bencineras(
    State(state): State<SharedState>,
    Query(b): Query<BoundsQuery>,
) -> impl IntoResponse {
    // ── 1. Validate ─────────────────────────────────────────────────────────
    if !is_valid_coord(b.south, b.west) || !is_valid_coord(b.north, b.east) {
        return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid bounds." }))).into_response();
    }

    // ── 2. Determine which tiles the bbox covers ─────────────────────────────
    let tile_origins = tiles_in_bbox(b.south, b.north, b.west, b.east);

    // ── 3. Classify tiles: cache hit vs need to fetch ────────────────────────
    let mut all_bencineras: Vec<Bencinera> = Vec::new();
    let mut tiles_to_fetch: Vec<(f64, f64)> = Vec::new();

    {
        let guard = state.tiles.read().await;
        for &(tlat, tlng) in &tile_origins {
            let key = tile_key(tlat, tlng);
            match guard.get(&key) {
                Some((data, ts)) if is_fresh(*ts) => {
                    all_bencineras.extend_from_slice(data);
                }
                _ => {
                    tiles_to_fetch.push((tlat, tlng));
                }
            }
        }
    }

    // ── 4. Fetch missing tiles sequentially with backoff ───────────────────────
    if !tiles_to_fetch.is_empty() {
        let mut newly_fetched: Vec<(String, Vec<Bencinera>)> = Vec::new();

        for (idx, &(tlat, tlng)) in tiles_to_fetch.iter().enumerate() {
            // Backoff: add slight delay between requests to avoid overwhelming API
            if idx > 0 {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            if let Some(bencineras) = fetch_tile(
                &state.client,
                tlat, tlat + TILE_SIZE,
                tlng, tlng + TILE_SIZE,
            ).await {
                all_bencineras.extend_from_slice(&bencineras);
                newly_fetched.push((tile_key(tlat, tlng), bencineras));
            }
        }

        if !newly_fetched.is_empty() {
            let mut guard = state.tiles.write().await;
            let ts = now_unix();
            guard.retain(|_, (_, t)| is_fresh(*t));
            for (key, bencineras) in newly_fetched {
                guard.insert(key, (bencineras, ts));
            }
            let snapshot = guard.clone();
            tokio::spawn(async move {
                save_cache_to_disk(snapshot).await;
            });
        }
    }

    // ── 5. Dedup by exact position, filter to original bbox ─────────────────
    all_bencineras.sort_by(|a, b| {
        a.lat.partial_cmp(&b.lat).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.lng.partial_cmp(&b.lng).unwrap_or(std::cmp::Ordering::Equal))
    });
    all_bencineras.dedup_by(|a, b| a.lat == b.lat && a.lng == b.lng);
    all_bencineras.retain(|item| {
        item.lat >= b.south && item.lat <= b.north
            && item.lng >= b.west && item.lng <= b.east
    });

    // ── 6. Enrich with CNE prices ──────────────────────────────────────────
    let cne = state.cne_stations.read().await;
    if !cne.is_empty() {
        for ben in &mut all_bencineras {
            if let Some(station) = nearest_cne_station(&cne, ben.lat, ben.lng, 0.3) {
                ben.precio_93 = station.precio_93.clone();
                ben.precio_95 = station.precio_95.clone();
                ben.precio_97 = station.precio_97.clone();
            }
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_valid_coord Tests ──────────────────────────────────────────────────

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

    // ── is_fresh Tests ────────────────────────────────────────────────────────

    #[test]
    fn is_fresh_recent_timestamp() {
        let now = now_unix();
        assert!(is_fresh(now));
        assert!(is_fresh(now - 1));
        assert!(is_fresh(now - 1000));
    }

    #[test]
    fn is_fresh_stale_timestamp() {
        let old_time = now_unix() - BENCINERA_TTL_SECS - 1;
        assert!(!is_fresh(old_time));
    }

    #[test]
    fn is_fresh_boundary_ttl() {
        let boundary = now_unix() - BENCINERA_TTL_SECS + 1;
        assert!(is_fresh(boundary), "should be fresh just before expiry");

        let expired = now_unix() - BENCINERA_TTL_SECS;
        assert!(!is_fresh(expired), "should be expired at TTL boundary");
    }

    // ── Tile Helper Tests ─────────────────────────────────────────────────────

    #[test]
    fn tile_origin_aligns_to_boundary() {
        // tile_origin should snap down to nearest TILE_SIZE (0.1)
        assert_eq!(tile_origin(0.15), 0.1);
        assert_eq!(tile_origin(0.19), 0.1);
        assert_eq!(tile_origin(0.20), 0.2);
        assert_eq!(tile_origin(-33.87), -33.9);
    }

    #[test]
    fn tile_key_formats_correctly() {
        let key = tile_key(-33.8, -70.7);
        assert_eq!(key, "-33.8:-70.7");

        let key = tile_key(0.0, 0.0);
        assert_eq!(key, "0.0:0.0");
    }

    #[test]
    fn tiles_in_bbox_single_tile() {
        // A small bbox that fits in one tile
        let tiles = tiles_in_bbox(-33.85, -33.80, -70.75, -70.70);
        assert!(!tiles.is_empty());
    }

    #[test]
    fn tiles_in_bbox_covers_span() {
        // A bbox that spans multiple tiles
        let tiles = tiles_in_bbox(-33.9, -33.5, -70.9, -70.5);
        // Should have multiple tiles covering this region
        assert!(tiles.len() > 1);
    }
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
