# TSP Solver

A Rust implementation of the Traveling Salesman Problem (TSP) solver using the 2-opt heuristic algorithm.

## Overview

This project provides a solution to the classic Traveling Salesman Problem, where the goal is to find the shortest possible route that visits each city exactly once and returns to the original city. The implementation uses the 2-opt algorithm, a local search technique that iteratively improves an initial tour by swapping edges to reduce the total distance.

## Features

- Generate random node configurations or specify custom node positions
- Implement multiple tour construction strategies:
  - Random tour generation
  - Nearest Neighbor heuristic
  - 2-opt optimization algorithm
- Visualize the resulting tour with a clean graphical output

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (2021 edition or later)
- Cargo (included with Rust)

### Installation

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/sfgarcia-tsp-solver.git
   cd sfgarcia-tsp-solver
   ```

2. Build the project:
   ```
   cargo build --release
   ```

### Usage

Run the program with:

```
cargo run --release
```

By default, this will:
1. Generate a random graph with 10 nodes
2. Apply the 2-opt algorithm to optimize the tour
3. Save the visualization as "graph.png" in the project root

## Customization

You can modify the behavior by editing the `main.rs` file:

- To use a fixed set of nodes instead of random ones, uncomment the `generate_graph()` function call
- To use the Nearest Neighbor algorithm, uncomment the `nearest_neighbour_tour()` line
- To use a completely random tour, uncomment the `random_tour()` line
- To change the number of random nodes, modify the first parameter in `Tour::create_random_nodes(10, 100.0, 100.0)`

## How It Works

### Tour Construction

The project implements multiple strategies for constructing tours:

1. **Random Tour**: A completely random ordering of nodes
2. **Nearest Neighbor**: A greedy algorithm that starts at a node and repeatedly visits the nearest unvisited node
3. **2-opt**: An optimization technique that improves a tour by reversing segments that would result in a shorter path

### 2-opt Algorithm

The 2-opt algorithm works by:
1. Starting with an initial tour
2. Considering all possible pairs of edges
3. Checking if swapping these edges would shorten the total distance
4. If so, making the swap and continuing until no more improvements can be made

### Visualization

The project uses the `plotters` crate to create a visual representation of the optimized route, showing:
- Nodes as red circles with their IDs
- Edges as black lines connecting the nodes

## Dependencies

- [plotters](https://crates.io/crates/plotters) (0.3.5) - For visualization
- [rand](https://crates.io/crates/rand) (0.8.5) - For random generation of nodes and tours
