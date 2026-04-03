# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release    # optimized build
cargo run --release      # run (outputs graph.png)
cargo test               # run tests
cargo clippy             # lint
cargo fmt                # format
cargo check              # fast syntax check
```

## Architecture

Three modules with a linear data flow:

```
main.rs  →  tour.rs  →  visualization.rs  →  graph.png
```

**`src/tour.rs`** — Core solver. Defines `Node` (city with x/y) and `Tour` (solver container). Key methods:
- `create_random_nodes(n, width, height)` — random city generation
- `nearest_neighbour_tour()` — greedy construction heuristic
- `two_opt()` — local search optimizer; iterates edge swaps until no improvement
- `distance_matrix()` — precomputes all pairwise Euclidean distances (used as cache)

**`src/main.rs`** — Entry point. Contains `generate_graph()` (fixed 5 nodes) and `generate_random_graph()` (10 random nodes, currently active in `main`).

**`src/visualization.rs`** — PNG rendering via `plotters`. Takes a solved route and outputs it scaled to the canvas with labeled nodes and edges.

## Planned Work (notes.txt)

- Add seed option for reproducible random generation
- Implement Lin-Kernighan algorithm as a better alternative to 2-opt
