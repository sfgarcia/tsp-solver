use rand::prelude::SliceRandom;
use rand::Rng;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: usize,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug)]
pub struct Tour {
    pub nodes: Vec<Node>,
    pub route: Vec<Node>,
    pub cost: f32,
    pub distance: Vec<Vec<Option<f32>>>,
}

impl Tour {
    pub fn new(positions: Vec<(f32, f32)>) -> Self {
        assert!(!positions.is_empty(), "Tour requires at least one node");
        let distance = vec![vec![None; positions.len()]; positions.len()];
        let mut nodes: Vec<Node> = Vec::new();
        let mut route: Vec<Node> = Vec::new();
        for (id, (x, y)) in positions.iter().enumerate() {
            route.push(Node { id, x: *x, y: *y });
            nodes.push(Node { id, x: *x, y: *y });
        }
        route.push(route[0].clone());
        Self { route, nodes, cost: 0.0, distance }
    }

    pub fn create_random_nodes(n: usize, width: f32, height: f32) -> Self {
        let mut rng = rand::thread_rng();
        let mut nodes: Vec<Node> = Vec::new();
        let mut route: Vec<Node> = Vec::new();
        for id in 0..n {
            let x = rng.gen::<f32>() * width;
            let y = rng.gen::<f32>() * height;
            route.push(Node {id, x, y });
            nodes.push(Node {id, x, y });
        }
        route.push(route[0].clone());
        let distance = vec![vec![None; nodes.len()]; nodes.len()];
        Self { nodes, route, cost: 0.0, distance }
    }

    fn distance(&self, node_1: &Node, node_2: &Node) -> f32 {
        if let Some(d) = self.distance[node_1.id][node_2.id] {
            return d;
        }
        if let Some(d) = self.distance[node_2.id][node_1.id] {
            return d;
        }
        haversine_km(node_1.x as f64, node_1.y as f64,
                     node_2.x as f64, node_2.y as f64) as f32
    }

    pub fn distance_matrix(&mut self) {
        for node_1 in self.nodes.iter() {
            for node_2 in self.nodes.iter() {
                if self.distance[node_1.id][node_2.id].is_none() {
                    let d = self.distance(node_1, node_2);
                    self.distance[node_1.id][node_2.id] = Some(d);
                }
            }
        }
    }

    pub fn calculate_cost(&mut self) {
        let mut cost = 0.0;
        for i in 0..self.route.len() - 1 {
            cost += self.distance(&self.route[i], &self.route[i + 1]);
        }
        self.cost = cost;
    }

    pub fn random_tour(&mut self) {
        let mut route: Vec<Node> = self.nodes[1..].to_vec();
        route.shuffle(&mut rand::thread_rng());
        self.route = vec![self.nodes[0].clone()];
        self.route.extend(route);
        self.route.push(self.nodes[0].clone());
    }

    pub fn nearest_neighbour_tour(&mut self) {
        self.distance_matrix();
        let mut current_index = self.nodes[0].id;
        let mut route = vec![self.nodes[0].clone()];
        let mut visited: HashMap<usize, bool> = HashMap::new();
        for node in self.nodes.iter() {
            visited.insert(node.id, false);
        }
        visited.insert(current_index, true);
        for _ in 0..self.nodes.len() - 1 {
            let mut dist: Vec<f32> = self.distance[current_index]
                .iter()
                .map(|d| d.unwrap_or(f32::MAX))
                .collect();
            for i in 0..self.nodes.len() {
                if visited[&i] {
                    dist[i] = f32::MAX;
                }
            }
            let min_index = dist.iter().enumerate().min_by(|x, y| x.1.partial_cmp(y.1).unwrap_or(std::cmp::Ordering::Equal)).unwrap().0;
            route.push(self.nodes[min_index].clone());
            // Mark the node as visited
            visited.insert(min_index, true);
            current_index = min_index;
        }
        route.push(self.nodes[0].clone());
        self.route = route;
    }

    pub fn two_opt(&mut self) {
        self.distance_matrix();
        let mut improved = true;
        while improved {
            improved = false;
            for i in 1..self.route.len() - 2 {
                for k in i + 1..self.route.len() - 1 {
                    let old_distance = self.distance[self.route[i - 1].id][self.route[i].id].unwrap_or(0.0)
                        + self.distance[self.route[k].id][self.route[k + 1].id].unwrap_or(0.0);
                    let new_distance = self.distance[self.route[i - 1].id][self.route[k].id].unwrap_or(0.0)
                        + self.distance[self.route[i].id][self.route[k + 1].id].unwrap_or(0.0);
                    let delta = old_distance - new_distance;
                    if delta > 0.0 {
                        self.route[i..=k].reverse();
                        improved = true;
                        break;
                    }
                }
                if improved {
                    break;
                }
            }
        }
    }

    // Or-opt: moves segments of size 1, 2, and 3 to a better position in the tour.
    // Runs after 2-opt to escape local optima that 2-opt cannot improve.
    pub fn or_opt(&mut self) {
        self.distance_matrix();
        let mut improved = true;
        while improved {
            improved = false;
            // n = number of cities (route has n+1 nodes, last == first)
            let n = self.route.len() - 1;
            'outer: for seg_size in 1..=3 {
                for i in 1..n - seg_size + 1 {
                    // Segment is route[i..i+seg_size]
                    let prev  = i - 1;
                    let last  = i + seg_size - 1;
                    let next  = i + seg_size; // may equal n (the closing node)

                    if next > n { continue; }

                    // Cost of removing the segment from its current position
                    let removal_gain =
                        self.distance[self.route[prev].id][self.route[i].id].unwrap_or(0.0)
                        + self.distance[self.route[last].id][self.route[next].id].unwrap_or(0.0)
                        - self.distance[self.route[prev].id][self.route[next].id].unwrap_or(0.0);

                    // Try inserting the segment after every other position j
                    for j in 1..n {
                        // Skip positions that overlap with the segment itself
                        if j >= prev && j <= last { continue; }

                        let j_next = if j + 1 > n { 1 } else { j + 1 };

                        let insertion_cost =
                            self.distance[self.route[j].id][self.route[i].id].unwrap_or(0.0)
                            + self.distance[self.route[last].id][self.route[j_next].id].unwrap_or(0.0)
                            - self.distance[self.route[j].id][self.route[j_next].id].unwrap_or(0.0);

                        if removal_gain - insertion_cost > 1e-6 {
                            // Rebuild route with segment relocated
                            let segment: Vec<Node> = self.route[i..=last].to_vec();
                            let mut new_route: Vec<Node> = Vec::with_capacity(self.route.len());

                            // Walk the original route skipping the segment,
                            // inserting it after position j
                            let mut k = 0;
                            while k <= n {
                                if k == i {
                                    k = last + 1; // skip segment
                                    continue;
                                }
                                new_route.push(self.route[k].clone());
                                // Insert segment after the adjusted j position
                                let adjusted_j = if j < i { j } else { j - seg_size };
                                if new_route.len() - 1 == adjusted_j {
                                    new_route.extend(segment.iter().cloned());
                                }
                                k += 1;
                            }

                            debug_assert_eq!(new_route.len(), self.route.len(), "or_opt rebuild produced wrong length");
                            self.route = new_route;
                            improved = true;
                            break 'outer;
                        }
                    }
                }
            }
        }
    }

}

fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    const R: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos()
        * (dlng / 2.0).sin().powi(2);
    R * 2.0 * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Haversine Distance Tests ───────────────────────────────────────────────

    #[test]
    fn haversine_london_to_paris() {
        // London (51.5074, -0.1278) → Paris (48.8566, 2.3522) ≈ 341 km
        let d = haversine_km(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((d - 341.0).abs() < 5.0, "expected ~341 km, got {:.1}", d);
    }

    #[test]
    fn haversine_zero_distance() {
        // Same point should be ~0 km
        let d = haversine_km(51.5, -0.1, 51.5, -0.1);
        assert!(d < 0.001, "expected ~0 km, got {:.6}", d);
    }

    #[test]
    fn haversine_symmetry() {
        // Distance A → B should equal B → A
        let d1 = haversine_km(51.5, -0.1, 48.8, 2.3);
        let d2 = haversine_km(48.8, 2.3, 51.5, -0.1);
        assert!((d1 - d2).abs() < 0.01, "symmetry failed: {:.2} vs {:.2}", d1, d2);
    }

    #[test]
    fn haversine_santiago_to_valparaiso() {
        // Santiago (-33.87, -70.72) → Valparaíso (-33.04, -71.63) ≈ 125 km
        let d = haversine_km(-33.87, -70.72, -33.04, -71.63);
        assert!((d - 125.0).abs() < 10.0, "expected ~125 km, got {:.1}", d);
    }

    // ── Tour Construction Tests ────────────────────────────────────────────────

    #[test]
    fn tour_new_creates_closed_loop() {
        // Tour with 3 nodes should have route[0] == route[last]
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 5.0)];
        let tour = Tour::new(positions);
        assert_eq!(tour.route.len(), 4, "route should have 4 nodes (3 + 1 closing)");
        assert_eq!(tour.route[0].id, tour.route[3].id);
    }

    #[test]
    fn tour_new_initializes_nodes() {
        let positions = vec![(1.0, 2.0), (3.0, 4.0)];
        let tour = Tour::new(positions);
        assert_eq!(tour.nodes.len(), 2);
        assert_eq!(tour.nodes[0].x, 1.0);
        assert_eq!(tour.nodes[0].y, 2.0);
    }

    #[test]
    fn tour_calculate_cost_single_point() {
        let positions = vec![(0.0, 0.0)];
        let mut tour = Tour::new(positions);
        tour.calculate_cost();
        assert!(tour.cost < 0.01, "single node cost should be ~0, got {}", tour.cost);
    }

    #[test]
    fn tour_calculate_cost_triangle() {
        // 3 nodes in a right triangle: (0,0) → (3,0) → (0,4) → (0,0)
        // Distances: 3 + 5 + 4 = 12 (Haversine approximates Euclidean for small distances)
        let positions = vec![(0.0, 0.0), (0.003, 0.0), (0.0, 0.004)];
        let mut tour = Tour::new(positions);
        tour.calculate_cost();
        // Cost should be positive (sum of edge weights)
        assert!(tour.cost > 0.0, "cost should be positive, got {}", tour.cost);
    }

    #[test]
    fn tour_distance_matrix_symmetric() {
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 5.0)];
        let mut tour = Tour::new(positions);
        tour.distance_matrix();

        // Distance[i][j] should equal distance[j][i]
        for i in 0..tour.nodes.len() {
            for j in 0..tour.nodes.len() {
                let d_ij = tour.distance[i][j];
                let d_ji = tour.distance[j][i];
                assert_eq!(d_ij, d_ji, "asymmetric distance: [{},{}]={:?} vs [{},{}]={:?}", i, j, d_ij, j, i, d_ji);
            }
        }
    }

    #[test]
    fn tour_distance_matrix_diagonal_zero() {
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (5.0, 5.0)];
        let mut tour = Tour::new(positions);
        tour.distance_matrix();

        // Distance from a node to itself should be ~0
        for i in 0..tour.nodes.len() {
            let d = tour.distance[i][i].unwrap_or(f32::NAN);
            assert!(d < 0.01, "diagonal distance should be ~0, got {}", d);
        }
    }

    // ── Nearest Neighbour Algorithm Tests ──────────────────────────────────────

    #[test]
    fn nearest_neighbour_visits_all_nodes() {
        let positions = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
        let mut tour = Tour::new(positions);
        tour.nearest_neighbour_tour();

        // Route should have n+1 nodes (n cities + 1 return to start)
        assert_eq!(tour.route.len(), 5, "route should visit all 4 nodes + return");

        // Should start and end at node 0
        assert_eq!(tour.route[0].id, 0);
        assert_eq!(tour.route[4].id, 0);

        // Should visit all intermediate nodes once
        let mut visited = vec![false; tour.nodes.len()];
        for i in 0..tour.route.len() - 1 {
            visited[tour.route[i].id] = true;
        }
        for v in visited.iter() {
            assert!(v, "not all nodes visited");
        }
    }

    #[test]
    fn nearest_neighbour_start_is_node_zero() {
        let positions = vec![(0.0, 0.0), (100.0, 100.0), (10.0, 10.0)];
        let mut tour = Tour::new(positions);
        tour.nearest_neighbour_tour();
        assert_eq!(tour.route[0].id, 0, "should start from node 0");
    }

    #[test]
    fn nearest_neighbour_three_nodes() {
        let positions = vec![(0.0, 0.0), (1.0, 0.0), (0.5, 0.5)];
        let mut tour = Tour::new(positions);
        tour.nearest_neighbour_tour();

        // Should have visited all 3 nodes + return to start
        assert_eq!(tour.route.len(), 4);
        assert_eq!(tour.route[0].id, 0);
        assert_eq!(tour.route[3].id, 0);

        // All nodes should be present in route[0..3]
        // (order depends on which neighbor is nearest from each node)
        let visited: Vec<usize> = tour.route[1..3].iter().map(|n| n.id).collect();
        assert!(visited.contains(&1));
        assert!(visited.contains(&2));
    }

    // ── 2-Opt Algorithm Tests ──────────────────────────────────────────────────

    #[test]
    fn two_opt_does_not_worsen_cost() {
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let mut tour = Tour::new(positions);
        tour.random_tour();
        tour.calculate_cost();
        let cost_before = tour.cost;

        tour.two_opt();
        tour.calculate_cost();
        let cost_after = tour.cost;

        assert!(cost_after <= cost_before + 0.1, "2-opt made route worse: {} → {}", cost_before, cost_after);
    }

    #[test]
    fn two_opt_preserves_all_nodes() {
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let mut tour = Tour::new(positions);
        tour.random_tour();
        tour.two_opt();

        // All nodes should still be in route (except closing duplicate)
        assert_eq!(tour.route.len(), 5);
        assert_eq!(tour.route[0].id, tour.route[4].id);
    }

    // ── Or-Opt Algorithm Tests ────────────────────────────────────────────────

    #[test]
    fn or_opt_preserves_route_length() {
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (5.0, 5.0)];
        let mut tour = Tour::new(positions);
        tour.random_tour();
        let len_before = tour.route.len();

        tour.or_opt();

        assert_eq!(tour.route.len(), len_before, "or_opt changed route length");
    }

    #[test]
    fn or_opt_keeps_all_nodes() {
        let positions = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (1.0, -1.0)];
        let mut tour = Tour::new(positions);
        tour.random_tour();

        // Record original nodes
        let original_ids: Vec<usize> = tour.route[..tour.route.len() - 1]
            .iter()
            .map(|n| n.id)
            .collect();

        tour.or_opt();

        // Check all nodes still present (minus the closing duplicate)
        let final_ids: Vec<usize> = tour.route[..tour.route.len() - 1]
            .iter()
            .map(|n| n.id)
            .collect();

        assert_eq!(final_ids.len(), original_ids.len());
        for id in original_ids.iter() {
            assert!(final_ids.contains(id), "node {} missing after or_opt", id);
        }
    }

    #[test]
    fn or_opt_does_not_worsen_cost() {
        let positions = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (5.0, 5.0)];
        let mut tour = Tour::new(positions);
        tour.random_tour();
        tour.calculate_cost();
        let cost_before = tour.cost;

        tour.or_opt();
        tour.calculate_cost();
        let cost_after = tour.cost;

        assert!(cost_after <= cost_before + 0.1, "or_opt made route worse: {} → {}", cost_before, cost_after);
    }

    // ── Random Tour Tests ──────────────────────────────────────────────────────

    #[test]
    fn random_tour_visits_all_nodes() {
        let positions = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)];
        let mut tour = Tour::new(positions);
        tour.random_tour();

        // Should have n+1 nodes in route (n cities + return)
        assert_eq!(tour.route.len(), 4);
        assert_eq!(tour.route[0].id, 0);
        assert_eq!(tour.route[3].id, 0);
    }

    // ── Edge Case Tests ────────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Tour requires at least one node")]
    fn tour_new_empty_positions_panics() {
        let positions: Vec<(f32, f32)> = vec![];
        Tour::new(positions);
    }
}