use crate::tour::Tour;
use crate::tour::Node;
use crate::visualization::plot_route;

pub mod tour;
pub mod visualization;

fn main(){
    let route = generate_random_graph();
    let _ = plot_route(&route, 640, 480, "graph.png");
}

fn generate_graph() -> Vec<Node> {
    let nodes = vec![
        (0.0, 0.0),
        (100.0, 50.0),
        (50.0, 100.0),
        (25.0, 25.0),
        (35.0, 50.0),
    ];
    let mut tour = Tour::new(nodes);
    //tour.nearest_neighbour_tour();
    //tour.random_tour();
    tour.two_opt();
    tour.route
}

fn generate_random_graph() -> Vec<Node> {
    let mut tour = Tour::create_random_nodes(10, 100.0, 100.0);
    //tour.nearest_neighbour_tour();
    tour.two_opt();
    tour.route
}
