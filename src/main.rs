use eframe::{egui::{self, Context}, run_native, App, CreationContext};
use egui_graphs::{Graph, GraphView, DefaultNodeShape, DefaultEdgeShape};
use petgraph::{stable_graph::StableGraph, Directed};

fn main() {
    let native_options = eframe::NativeOptions::default();
    run_native(
        "egui_graphs_basic_demo",
        native_options,
        Box::new(|cc| Box::new(BasicApp::new(cc))),
    )
    .unwrap();
}


pub struct BasicApp {
    g: Graph<(), (), Directed>,
}

impl BasicApp {
    fn new(_: &CreationContext<'_>) -> Self {
        let g = generate_graph();
        Self { g: Graph::from(&g) }
    }
}

fn generate_graph() -> StableGraph<(), (), Directed> {
    let mut g: StableGraph<(), ()> = StableGraph::new();

    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());

    g.add_edge(a, b, ());
    g.add_edge(b, c, ());
    g.add_edge(c, a, ());

    g
}

impl App for BasicApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(&mut GraphView::<
                _,
                _,
                _,
                _,
                DefaultNodeShape,
                DefaultEdgeShape,
            >::new(&mut self.g));
        });
    }
}