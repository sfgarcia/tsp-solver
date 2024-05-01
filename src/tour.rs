use petgraph::graph::Graph;
use petgraph::Directed;
use rand::prelude::SliceRandom;

#[derive(Debug, Clone)]
pub struct Node {
    pub x: f32,
    pub y: f32,
}

pub struct Tour {
  pub nodes: Vec<Node>,
  pub graph: Graph<Node, (), Directed>,
  pub cost: f32,
}

impl Tour {
  pub fn new(nodes: Vec<Node>) -> Self {
      let mut graph: Graph<Node, ()> = Graph::new();
      for node in nodes.iter() {
          graph.add_node(node.clone());
      }
      Self { nodes, graph, cost: 0.0 }
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

  pub fn create_random_tour(&mut self) {
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
}