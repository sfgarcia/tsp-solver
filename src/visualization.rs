use plotters::coord::Shift;
use plotters::prelude::*;
use plotters::backend::RGBPixel;
use plotters::coord::types::RangedCoordf32;
use crate::tour::Node;


pub fn plot_route(route: &Vec<Node>, width: u32, height: u32, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(path, (width, height)).into_drawing_area();
    root.fill(&WHITE)?;

    // Find the minimum and maximum x and y values in the graph
    let min_x = route.iter().map(|node| node.x).fold(f32::INFINITY, |a, b| a.min(b));
    let max_x = route.iter().map(|node| node.x).fold(f32::NEG_INFINITY, |a, b| a.max(b));
    let min_y = route.iter().map(|node| node.y).fold(f32::INFINITY, |a, b| a.min(b));
    let max_y = route.iter().map(|node| node.y).fold(f32::NEG_INFINITY, |a, b| a.max(b));

    let mut chart = ChartBuilder::on(&root)
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(min_x..max_x, min_y..max_y)?;

    draw_edges(&mut chart, &route)?;

    // Add a margin to the drawing area
    let margin = 40;
    let drawing_width = width - 2 * margin;
    let drawing_height = height - 2 * margin;

    draw_nodes(&root, &mut chart, &route, min_x, max_x, min_y, max_y, drawing_width, drawing_height, margin, height)?;
    Ok(())
}

fn draw_edges(chart: &mut ChartContext<BitMapBackend<RGBPixel>, Cartesian2d<RangedCoordf32, RangedCoordf32>>, route: &Vec<Node>) -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..route.len() - 1 {
        // Draw the edge as a line between the start node and the end node
        chart.draw_series(LineSeries::new(
            vec![(route[i].x, route[i].y), (route[i + 1].x, route[i + 1].y)],
            &BLACK,
        ))?;
    }
    Ok(())
}

fn draw_nodes(
        root: &DrawingArea<BitMapBackend<RGBPixel>, Shift>,
        chart: &mut ChartContext<BitMapBackend<RGBPixel>, Cartesian2d<RangedCoordf32, RangedCoordf32>>,
        route: &Vec<Node>,
        min_x: f32,
        max_x: f32,
        min_y: f32,
        max_y: f32,
        drawing_width: u32,
        drawing_height: u32,
        margin: u32,
        height: u32,
        ) -> Result<(), Box<dyn std::error::Error>> {
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
