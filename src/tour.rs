use petgraph::graph::{Graph, NodeIndex};
use petgraph::Directed;
use rand::prelude::SliceRandom;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct Node {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug)]
pub struct Tour {
    pub nodes: Vec<Node>,
    pub graph: Graph<Node, (), Directed>,
    pub cost: f32,
    pub distance: Vec<Vec<f32>>,
}

impl Tour {
    pub fn new(nodes: Vec<Node>) -> Self {
        let mut graph: Graph<Node, ()> = Graph::new();
        for node in nodes.iter() {
        graph.add_node(node.clone());
        }
        let mut distance = vec![vec![0.0; nodes.len()]; nodes.len()];
        Self { nodes, graph, cost: 0.0, distance }
    }

    pub fn create_random_nodes(n: usize, width: f32, height: f32) -> Self {
        let mut rng = rand::thread_rng();
        let mut graph: Graph<Node, ()> = Graph::new();
        let mut nodes: Vec<Node> = Vec::new();
        for _ in 0..n {
            let x = rng.gen::<f32>() * width;
            let y = rng.gen::<f32>() * height;
            nodes.push(Node { x, y });
            graph.add_node(Node { x, y });
        }
        let distance = vec![vec![0.0; nodes.len()]; nodes.len()];
        Self { nodes, graph, cost: 0.0, distance }
    }

    fn add_edge(&mut self, a: usize, b: usize) {
        let a = self.graph.node_indices().nth(a).unwrap();
        let b = self.graph.node_indices().nth(b).unwrap();
        self.graph.add_edge(a, b, ());
    }

    fn add_edges(&mut self, edges: Vec<(usize, usize)>) {
        for (a, b) in edges {
            self.add_edge(a, b);
        }
    }

    fn distance(&self, a: usize, b: usize) -> f32 {
        let node_a = &self.nodes[a];
        let node_b = &self.nodes[b];
        ((node_a.x - node_b.x).powi(2) + (node_a.y - node_b.y).powi(2)).sqrt()
    }

    fn get_node(&self, index: usize) -> NodeIndex {
        self.graph.node_indices().nth(index).unwrap()
    }

    pub fn distance_matrix(&mut self) {
        for i in 0..self.nodes.len() {
            for j in 0..self.nodes.len() {
                self.distance[i][j] = self.distance(i, j);
            }
        }
    }

    fn calculate_cost(&mut self) {
        let mut cost = 0.0;
        for edge in self.graph.edge_indices() {
            let (start_node, end_node) = self.graph.edge_endpoints(edge).unwrap();
            cost += self.distance(start_node.index(), end_node.index());
        }
        self.cost = cost;
    }

    pub fn random_tour(&mut self) {
        let mut indices: Vec<usize> = (1..self.nodes.len()).collect();
        indices.shuffle(&mut rand::thread_rng());
        // Remove and return the first element of the indices vector
        let tour_init = 0;
        let mut current_index = tour_init;
        while indices.len() > 0 {
            let next_index = indices.remove(0);
            self.add_edge(current_index, next_index);
            current_index = next_index;
        }
        self.add_edge(current_index, tour_init);
    }

    pub fn nearest_neighbour_tour(&mut self) {
        self.distance_matrix();
        let tour_init = 0;
        let mut current_index = tour_init;
        let mut visited = vec![false; self.nodes.len()];
        visited[current_index] = true;
        for _ in 0..self.nodes.len() - 1 {
            let mut dist: Vec<f32> = self.distance[current_index].clone();
            for i in 0..self.nodes.len() {
                if visited[i] {
                    dist[i] = f32::MAX;
                }
            }
            let min_index = dist.iter().enumerate().min_by(|x, y| x.1.partial_cmp(y.1).unwrap()).unwrap().0;
            self.add_edge(current_index, min_index);
            visited[min_index] = true;
            current_index = min_index;
        }
        self.add_edge(current_index, tour_init);
    }
}