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
    pub distance: Vec<Vec<f32>>,
}

impl Tour {
    pub fn new(positions: Vec<(f32, f32)>) -> Self {
        let distance = vec![vec![0.0; positions.len()]; positions.len()];
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
        let distance = vec![vec![0.0; nodes.len()]; nodes.len()];
        Self { nodes, route, cost: 0.0, distance }
    }

    fn distance(&self, node_1: &Node, node_2: &Node) -> f32 {
        if self.distance[node_1.id][node_2.id] != 0.0 {
            return self.distance[node_1.id][node_2.id];
        }
        else if self.distance[node_2.id][node_1.id] != 0.0 {
            return self.distance[node_2.id][node_1.id];
        }
        ((node_1.x - node_2.x).powi(2) + (node_1.y - node_2.y).powi(2)).sqrt()
    }

    pub fn distance_matrix(&mut self) {
        for node_1 in self.nodes.iter() {
            for node_2 in self.nodes.iter() {
                self.distance[node_1.id][node_2.id] = self.distance(node_1, node_2);
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
            let mut dist: Vec<f32> = self.distance[current_index].clone();
            for i in 0..self.nodes.len() {
                if visited[&i] {
                    dist[i] = f32::MAX;
                }
            }
            let min_index = dist.iter().enumerate().min_by(|x, y| x.1.partial_cmp(y.1).unwrap()).unwrap().0;
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
                    let old_distance = self.distance[self.route[i - 1].id][self.route[i].id] + self.distance[self.route[k].id][self.route[k + 1].id];
                    let new_distance = self.distance[self.route[i - 1].id][self.route[k].id] + self.distance[self.route[i].id][self.route[k + 1].id];
                    let delta = old_distance - new_distance;
                    if delta > 0.0 {
                        let new_route = self.route[0..i].to_vec()
                            .into_iter()
                            .chain(self.route[i..k + 1].to_vec().into_iter().rev())
                            .chain(self.route[k + 1..self.route.len()].to_vec().into_iter())
                            .collect();
                        self.route = new_route;
                        improved = true;
                    }
                }
            }
        }
    }

}