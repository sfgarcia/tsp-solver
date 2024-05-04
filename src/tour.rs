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
    pub fn new(nodes: Vec<Node>) -> Self {
        let distance = vec![vec![0.0; nodes.len() - 1]; nodes.len() - 1];
        let mut route = nodes.clone();
        route.push(nodes[0].clone());
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
        let distance = vec![vec![0.0; route.len() - 1]; route.len() - 1];
        Self { nodes, route, cost: 0.0, distance }
    }

    fn distance(&self, node_1: &Node, node_2: &Node) -> f32 {
        ((node_1.x - node_2.x).powi(2) + (node_1.y - node_2.y).powi(2)).sqrt()
    }

    pub fn distance_matrix(&mut self) {
        for node_1 in self.nodes.iter() {
            for node_2 in self.nodes.iter() {
                self.distance[node_1.id][node_2.id] = self.distance(node_1, node_2);
            }
        }
    }

    fn calculate_cost(&mut self) {
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

    /*
    pub fn two_opt(&mut self) {
        let mut improved = true;
        while improved {
            improved = false;
            for (i, edge1) in self.graph.edge_indices().enumerate() {
                for edge2 in self.graph.edge_indices().skip(i + 1) {
                    let (start_node1, end_node1) = self.graph.edge_endpoints(edge1).unwrap();
                    let (start_node2, end_node2) = self.graph.edge_endpoints(edge2).unwrap();
                    let cost1 = self.distance(start_node1.index(), end_node1.index()) + self.distance(start_node2.index(), end_node2.index());
                    let cost2 = self.distance(start_node1.index(), start_node2.index()) + self.distance(end_node1.index(), end_node2.index());
                    if cost2 < cost1 {
                        self.graph.remove_edge(edge1);
                        self.graph.remove_edge(edge2);
                        self.add_edge(start_node1.index(), start_node2.index());
                        self.add_edge(end_node1.index(), end_node2.index());
                        improved = true;
                        //println!("Removed edges ({}, {}) and ({}, {}) and added edges ({}, {}) and ({}, {})", start_node1.index(), end_node1.index(), start_node2.index(), end_node2.index(), start_node1.index(), start_node2.index(), end_node1.index(), end_node2.index());
                    }
                }
            }
        }
    }

    pub fn two_opt_swap(&mut self, i: usize, k: usize) {
        let mut new_tour = Vec::new();
        for j in 0..i {
            new_tour.push(self.get_node(j));
        }
        for j in (i..k + 1).rev() {
            new_tour.push(self.get_node(j));
        }
        for j in k + 1..self.nodes.len() {
            new_tour.push(self.get_node(j));
        }
        // Create a new graph with the new tour
        let mut new_graph: Graph<Node, ()> = Graph::new();
        for node in new_tour.iter() {
            new_graph.add_node(self.graph[*node].clone());
        }
        for i in 0..new_tour.len() {
            new_graph.add_edge(new_tour[i], new_tour[(i + 1) % new_tour.len()], ());
        }
        self.graph = new_graph;
    }
     */
}