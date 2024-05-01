use plotters::prelude::*;
use petgraph::graph::Graph;
use petgraph::Directed;
use crate::tour::Tour;
use crate::tour::Node;

pub mod tour;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (640, 480);
    let root = BitMapBackend::new("graph.png", (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let graph = generate_graph();

    // Find the minimum and maximum x and y values in the graph
    let min_x = graph.node_indices().map(|i| graph[i].x).fold(f32::INFINITY, f32::min);
    let max_x = graph.node_indices().map(|i| graph[i].x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = graph.node_indices().map(|i| graph[i].y).fold(f32::INFINITY, f32::min);
    let max_y = graph.node_indices().map(|i| graph[i].y).fold(f32::NEG_INFINITY, f32::max);

    // Add a margin to the drawing area
    let margin = 40;
    let drawing_width = width - 2 * margin;
    let drawing_height = height - 2 * margin;

    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(min_x..max_x, min_y..max_y)?;

    for edge in graph.edge_indices() {
        let (start_node, end_node) = graph.edge_endpoints(edge).unwrap();
        let start_point = &graph[start_node];
        let end_point = &graph[end_node];

        // Draw the edge as a line between the start node and the end node
        chart.draw_series(LineSeries::new(
            vec![(start_point.x, start_point.y), (end_point.x, end_point.y)],
            &BLACK,
        ))?;
    }

    for (i, node) in graph.node_indices().enumerate() {
        let point = &graph[node];
        chart.draw_series(PointSeries::of_element(
            [(point.x, point.y)],
            5,
            ShapeStyle::from(&RED).filled(),
            &|coord, size, style| {
                EmptyElement::at(coord)
                    + Circle::new((0, 0), size, style)
            },
        ))?;

        // Add labels to the nodes
        let label_coord = ((point.x - min_x) / (max_x - min_x) * drawing_width as f32 + margin as f32, (point.y - min_y) / (max_y - min_y) * drawing_height as f32 + margin as f32);
        let text_style = TextStyle::from(("Arial", 15).into_font()).color(&BLACK);
        root.draw_text(&format!("Node {}", i), &text_style, (label_coord.0 as i32, (height as f32 - label_coord.1) as i32 - 20))?;
    }

    Ok(())
}

fn generate_graph() -> Graph<Node, (), Directed> {
    let nodes = vec![
        Node { x: 0.0, y: 0.0 },
        Node { x: 100.0, y: 50.0 },
        Node { x: 50.0, y: 100.0 },
        Node { x: 25.0, y: 25.0 },
        Node { x: 35.0, y: 50.0 },
    ];
    let mut tour = Tour::new(nodes);
    tour.random_tour();
    let matrix = tour.distance_matrix();
    println!("{:?}", matrix);

    tour.graph
}

fn generate_random_graph() -> Graph<Node, (), Directed> {
    /*
    let nodes = vec![
        Node { x: 0.0, y: 0.0 },
        Node { x: 100.0, y: 50.0 },
        Node { x: 50.0, y: 100.0 },
        Node { x: 25.0, y: 25.0 },
        Node { x: 35.0, y: 50.0 },
    ];
    let mut tour = Tour::new(nodes);
    */
    let mut tour = Tour::create_random_nodes(100, 100.0, 100.0);
    tour.random_tour();

    tour.graph
}
