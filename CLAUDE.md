# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Purpose

Web service for solving Traveling Salesman Problems (TSP) with real-world geolocation data. Built with Rust/Axum, uses Overpass API to fetch gas stations (bencineras) in Chile, solves via nearest-neighbor + 2-opt + or-opt algorithms.

**Use case**: Given a bounding box in Santiago or other Chilean regions, fetch all nearby gas stations and compute optimal route to visit them all.

## Setup & Commands

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build & run
cargo build --release      # optimized build
cargo run --release        # run server on http://0.0.0.0:3000

# Tests
cargo test                 # run all tests (in headless mode)
cargo test -- --nocapture # run with output

# Lint & format
cargo clippy               # lint
cargo fmt                  # auto-format
cargo check                # fast syntax check
```

## Architecture

### Modules

| Module | Role |
|--------|------|
| **main.rs** | Axum server entry point. Sets up routes: `/solve` (POST TSP), `/bencineras` (GET gas stations), `/` (static HTML), `/manifest.json`, `/sw.js`, `/icon.svg` |
| **handlers.rs** | HTTP request handlers + Overpass API integration. Tile-based caching with TTL (86400s). Fetches bencineras in parallel from 3 Overpass mirrors |
| **tour.rs** | Core TSP algorithms. `Tour` struct, `Node` (lat/lng), nearest-neighbor construction, 2-opt + or-opt local search, haversine distance |

### Data Flow

```
Browser: POST /solve with coordinates
  ↓
handlers::solve() validates input (3-200 nodes, valid lat/lng)
  ↓
spawn_blocking { Tour::new() → nearest_neighbour_tour() → two_opt() → or_opt() → calculate_cost() }
  ↓
Timeout: 30 seconds (SOLVER_TIMEOUT_SECS)
  ↓
SolveResponse: route (Vec<RoutePoint>), total_distance_km
```

### TSP Solver Algorithm

**Initialization**: `nearest_neighbour_tour()` (greedy: always visit nearest unvisited city)  
**Improvement**: `two_opt()` (swap edges to uncross tours)  
**Escape local optima**: `or_opt()` (move 1-3 city segments to better positions)

**Distance metric**: Haversine (great-circle distance on Earth in km)

### Bencineras (Gas Stations) API

**Caching strategy**: Tile-based (0.1° × 0.1° ≈ 11 km squares).
- Each tile key: `"{lat:.1}:{lng:.1}"`
- TTL: 86400 seconds (24 hours)
- Fetched in parallel from 3 Overpass mirrors (kumi.systems, overpass-api.de, private.coffee)
- Persisted to `bencineras_cache.json`

**Legacy data cleanup**: Drops old bbox-format ("south:north:west:east") and commune-format keys on load.

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_NODES` | 200 | Max coordinates in solve request |
| `SOLVER_TIMEOUT_SECS` | 30 | Max time for TSP solver |
| `BENCINERA_TTL_SECS` | 86400 | Cache freshness (24h) |
| `TILE_SIZE` | 0.1° | Tile grid size (~11 km) |

## Testing

Run all tests (no external dependencies):
```bash
cargo test --lib
```

Test categories:
- **tour.rs**: Algorithm tests (haversine distance, tour construction, optimization, payback detection)
- **handlers.rs**: Validation & helper function tests (coord validation, freshness checks)
- **Integration**: None (would require real Overpass API calls)

## Code Quality

### Known Issues

1. **Unwrap calls in main.rs** (lines 27, 39, 45, 62, 64)  
   Should use `.expect("descriptive message")` for better panic messages.

2. **Visualization.rs is dead code**  
   No longer compiled (plotters not in Cargo.toml). Should remove.

3. **nearest_neighbour_tour() unused**  
   Currently solver uses `random_tour()` → `two_opt()`. Should use `nearest_neighbour_tour()` as initial solution (better starting point).

4. **precision loss in handlers.rs:339**  
   `n.x as f64` loses f32 precision. Add comment explaining acceptable error margin.

### Standards

- **Type safety**: Strong (no unsafe blocks, proper type system usage)
- **Error handling**: Use `.expect()` with descriptive messages instead of `.unwrap()`
- **Testing**: Unit tests in `#[cfg(test)]` blocks within each module
- **Linting**: Run `cargo clippy` before commit
- **Formatting**: Use `cargo fmt` before commit
