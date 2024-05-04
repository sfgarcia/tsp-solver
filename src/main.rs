use plotters::prelude::*;
use crate::tour::Tour;
use crate::tour::Node;

pub mod tour;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (640, 480);
    let root = BitMapBackend::new("graph.png", (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    let route = generate_random_graph();

    // Find the minimum and maximum x and y values in the graph
    let min_x = route.iter().map(|node| node.x).fold(f32::INFINITY, |a, b| a.min(b));
    let max_x = route.iter().map(|node| node.x).fold(f32::NEG_INFINITY, |a, b| a.max(b));
    let min_y = route.iter().map(|node| node.y).fold(f32::INFINITY, |a, b| a.min(b));
    let max_y = route.iter().map(|node| node.y).fold(f32::NEG_INFINITY, |a, b| a.max(b));

    // Add a margin to the drawing area
    let margin = 40;
    let drawing_width = width - 2 * margin;
    let drawing_height = height - 2 * margin;

    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(min_x..max_x, min_y..max_y)?;

    for i in 0..route.len() - 1 {
        // Draw the edge as a line between the start node and the end node
        chart.draw_series(LineSeries::new(
            vec![(route[i].x, route[i].y), (route[i + 1].x, route[i + 1].y)],
            &BLACK,
        ))?;
    }

    for node in route.iter() {
        chart.draw_series(PointSeries::of_element(
            [(node.x, node.y)],
            5,
            ShapeStyle::from(&RED).filled(),
            &|coord, size, style| {
                EmptyElement::at(coord)
                    + Circle::new((0, 0), size, style)
            },
        ))?;

        // Add labels to the nodes
        let label_coord = ((node.x - min_x) / (max_x - min_x) * drawing_width as f32 + margin as f32, (node.y - min_y) / (max_y - min_y) * drawing_height as f32 + margin as f32);
        let text_style = TextStyle::from(("Arial", 15).into_font()).color(&BLACK);
        root.draw_text(&format!("Node {}", node.id), &text_style, (label_coord.0 as i32, (height as f32 - label_coord.1) as i32 - 20))?;
    }

    Ok(())
}

fn generate_graph() -> Vec<Node> {
    let nodes = vec![
        Node { id: 0, x: 0.0, y: 0.0 },
        Node { id: 1, x: 100.0, y: 50.0 },
        Node { id: 2, x: 50.0, y: 100.0 },
        Node { id: 3, x: 25.0, y: 25.0 },
        Node { id: 4, x: 35.0, y: 50.0 },
    ];
    let mut tour = Tour::new(nodes);
    //tour.random_tour();
    //tour.two_opt();
    tour.route
}

fn generate_random_graph() -> Vec<Node> {
    let mut tour = Tour::create_random_nodes(10, 100.0, 100.0);
    tour.nearest_neighbour_tour();
    //tour.two_opt();
    tour.route
}
