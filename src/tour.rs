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

    #[test]
    fn haversine_london_to_paris() {
        // London (51.5074, -0.1278) → Paris (48.8566, 2.3522) ≈ 341 km
        let d = haversine_km(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((d - 341.0).abs() < 5.0, "expected ~341 km, got {:.1}", d);
    }
}