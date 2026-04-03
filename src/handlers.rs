use axum::{extract::{Json, Query, State}, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use crate::tour::Tour;

// ── Constants ──────────────────────────────────────────────────────────────────

const MAX_NODES: usize = 200;
const SOLVER_TIMEOUT_SECS: u64 = 30;
const OSRM_TIMEOUT_SECS: u64 = 10;
const OSRM_MAX_COORDS: usize = 100; // public demo server limit
const OSRM_BASE: &str = "http://router.project-osrm.org/table/v1/driving";

// ── Validation ────────────────────────────────────────────────────────────────

fn is_valid_coord(lat: f64, lng: f64) -> bool {
    lat.is_finite() && lng.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lng)
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

pub async fn login_cne(client: &reqwest::Client, email: &str, password: &str) -> Option<String> {
    let resp = client
        .post("https://api.cne.cl/api/login")
        .form(&[("email", email), ("password", password)])
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    json["token"].as_str().map(|s| s.to_string())
}

// ── OSRM Road-Time Matrix ─────────────────────────────────────────────────────

/// Parses the `durations` array from an OSRM table response into an NxN matrix (seconds).
/// Returns `None` if the key is missing or the structure is malformed.
/// Unreachable pairs (JSON `null`) become `f32::MAX`.
fn parse_osrm_durations(body: &serde_json::Value) -> Option<Vec<Vec<f32>>> {
    body["durations"].as_array()?.iter().map(|row| {
        row.as_array().map(|r| {
            // null entries (unreachable pairs) become f32::MAX — not f64::MAX as f32 which overflows to inf
            r.iter().map(|v| v.as_f64().map_or(f32::MAX, |x| x as f32)).collect()
        })
    }).collect()
}

/// Fetches an NxN travel-time matrix (seconds) from the OSRM public table API.
/// Returns `None` on timeout, HTTP error, or malformed response — caller falls back to haversine.
async fn fetch_osrm_matrix(client: &reqwest::Client, coords: &[(f64, f64)]) -> Option<Vec<Vec<f32>>> {
    // OSRM expects lng,lat order
    let coord_str = coords.iter()
        .map(|(lat, lng)| format!("{},{}", lng, lat))
        .collect::<Vec<_>>()
        .join(";");
    let url = format!("{}/{}?annotations=duration", OSRM_BASE, coord_str);
    let resp = tokio::time::timeout(
        Duration::from_secs(OSRM_TIMEOUT_SECS),
        client.get(&url).send(),
    ).await.ok()?.ok()?;
    if !resp.status().is_success() { return None; }
    let body: serde_json::Value = resp.json().await.ok()?;
    parse_osrm_durations(&body)
}

pub async fn fetch_cne_stations(client: &reqwest::Client, token: &str) -> Vec<CneStation> {
    let resp = match client
        .get("https://api.cne.cl/api/v4/estaciones")
        .bearer_auth(token)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => { eprintln!("CNE fetch error: {e}"); return Vec::new(); }
    };

    let status = resp.status();
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => { eprintln!("CNE read error: {e}"); return Vec::new(); }
    };

    // Try to decompress gzip manually if needed
    let json: serde_json::Value = if bytes.starts_with(b"\x1f\x8b") {
        // gzip magic bytes — decompress manually
        use std::io::Read;
        let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();
        if decoder.read_to_end(&mut decompressed).is_err() {
            eprintln!("CNE gzip decompress error");
            return Vec::new();
        }
        match serde_json::from_slice(&decompressed) {
            Ok(v) => v,
            Err(e) => { eprintln!("CNE parse error after decompress: {e}"); return Vec::new(); }
        }
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => { eprintln!("CNE parse error (status={status}): {e}"); return Vec::new(); }
        }
    };

    json.as_array()
        .map(|arr| arr.iter().filter_map(parse_cne_station).collect())
        .unwrap_or_else(|| { eprintln!("CNE response is not an array (status={status})"); Vec::new() })
}

pub async fn status(State(state): State<SharedState>) -> impl IntoResponse {
    let count = state.cne_stations.read().await.len();
    Json(serde_json::json!({ "cne_stations": count }))
}

pub async fn debug_cne(State(state): State<SharedState>) -> impl IntoResponse {
    let resp = state.client
        .get("https://api.cne.cl/api/v4/estaciones")
        .bearer_auth("test")
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            Json(serde_json::json!({
                "http_status": status,
                "body_preview": &body[..body.len().min(200)]
            }))
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_distance_km: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_travel_time_secs: Option<f32>,
    /// "osrm" when road times were used, "haversine" on fallback
    pub routing: &'static str,
}

#[derive(Serialize)]
pub struct RoutePoint {
    pub lat: f64,
    pub lng: f64,
}

pub async fn solve(
    State(state): State<SharedState>,
    Json(payload): Json<SolveRequest>,
) -> impl IntoResponse {
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

    let coords: Vec<(f64, f64)> = payload.coordinates.iter().map(|c| (c.lat, c.lng)).collect();
    let osrm_matrix = if coords.len() <= OSRM_MAX_COORDS {
        fetch_osrm_matrix(&state.client, &coords).await
    } else {
        None
    };
    let use_osrm = osrm_matrix.is_some();

    let result = tokio::time::timeout(
        Duration::from_secs(SOLVER_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            let mut tour = if let Some(matrix) = osrm_matrix {
                Tour::with_matrix(positions, matrix)
            } else {
                Tour::new(positions)
            };
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

    let (total_distance_km, total_travel_time_secs, routing) = if use_osrm {
        (None, Some(tour.cost), "osrm")
    } else {
        (Some(tour.cost), None, "haversine")
    };

    (
        StatusCode::OK,
        Json(SolveResponse { route, total_distance_km, total_travel_time_secs, routing }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_osrm_durations Tests ────────────────────────────────────────────

    #[test]
    fn parse_osrm_durations_should_return_matrix_from_valid_response() {
        let body = serde_json::json!({
            "durations": [[0.0, 120.5, 300.0], [130.0, 0.0, 180.0], [310.0, 185.0, 0.0]]
        });
        let matrix = parse_osrm_durations(&body).unwrap();
        assert_eq!(matrix.len(), 3);
        assert!((matrix[0][1] - 120.5).abs() < 0.01, "expected 120.5, got {}", matrix[0][1]);
    }

    #[test]
    fn parse_osrm_durations_should_preserve_asymmetry() {
        let body = serde_json::json!({
            "durations": [[0.0, 120.5, 300.0], [130.0, 0.0, 180.0], [310.0, 185.0, 0.0]]
        });
        let matrix = parse_osrm_durations(&body).unwrap();
        assert!((matrix[0][1] - 120.5).abs() < 0.01);
        assert!((matrix[1][0] - 130.0).abs() < 0.01);
        assert_ne!(matrix[0][1], matrix[1][0], "asymmetric values should differ");
    }

    #[test]
    fn parse_osrm_durations_should_coerce_null_to_max() {
        let body = serde_json::json!({
            "durations": [[0.0, null], [null, 0.0]]
        });
        let matrix = parse_osrm_durations(&body).unwrap();
        assert_eq!(matrix[0][1], f32::MAX);
        assert_eq!(matrix[1][0], f32::MAX);
    }

    #[test]
    fn parse_osrm_durations_should_return_none_when_key_missing() {
        let body = serde_json::json!({ "code": "Ok" });
        assert!(parse_osrm_durations(&body).is_none());
    }

    #[test]
    fn parse_osrm_durations_should_return_none_on_empty_object() {
        let body = serde_json::json!({});
        assert!(parse_osrm_durations(&body).is_none());
    }

    #[test]
    fn parse_osrm_durations_should_preserve_matrix_dimensions() {
        let body = serde_json::json!({
            "durations": [[0.0, 1.0, 2.0], [3.0, 0.0, 4.0], [5.0, 6.0, 0.0]]
        });
        let matrix = parse_osrm_durations(&body).unwrap();
        assert_eq!(matrix.len(), 3);
        assert!(matrix.iter().all(|row| row.len() == 3));
    }

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
