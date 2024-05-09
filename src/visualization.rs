use plotters::coord::Shift;
use plotters::prelude::*;
use plotters::backend::RGBPixel;
use plotters::coord::types::RangedCoordf32;
use crate::tour::Node;

pub struct Visualization<'a> {
    pub width: u32,
    pub height: u32,
    pub margin: u32,
    pub drawing_width: u32,
    pub drawing_height: u32,
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub chart: ChartContext<'a, BitMapBackend<'a, RGBPixel>, Cartesian2d<RangedCoordf32, RangedCoordf32>>,
    route: Vec<Node>,
}

impl<'a> Visualization<'a> {
    pub fn new(root: DrawingArea<BitMapBackend<'static, RGBPixel>, Shift>, width: u32, height: u32, margin: u32, route: Vec<Node>) -> Self {
        root.fill(&WHITE).unwrap();

        // Find the minimum and maximum x and y values in the graph
        let min_x = route.iter().map(|node| node.x).fold(f32::INFINITY, |a, b| a.min(b));
        let max_x = route.iter().map(|node| node.x).fold(f32::NEG_INFINITY, |a, b| a.max(b));
        let min_y = route.iter().map(|node| node.y).fold(f32::INFINITY, |a, b| a.min(b));
        let max_y = route.iter().map(|node| node.y).fold(f32::NEG_INFINITY, |a, b| a.max(b));

        let chart = ChartBuilder::on(&root.clone())
            .caption("Travelling Salesman Problem", ("Arial", 30).into_font())
            .x_label_area_size(40)
            .y_label_area_size(40)
            .build_cartesian_2d(min_x..max_x, min_y..max_y)
            .unwrap();
        Self { width, height, margin, drawing_width: width - 2 * margin, drawing_height: height - 2 * margin, min_x, max_x, min_y, max_y, chart, route }
    }
}
