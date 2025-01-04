use clap::Parser;
use graphviz_rust::parse;
use graphviz_rust::printer::{DotPrinter, PrinterContext};
use std::fs::read_to_string;
use std::path::PathBuf;
use std::{fs, io};
use graphviz_rust::dot_structures::Graph;
use log::warn;
use inv_call_extract::{get_id_str, TypedGraph};
use crate::linker::object_file::ObjectFile;

pub mod linker;

/// Program that builds inverse call graph with required functions only.
/// It can be used for creating new .dot graph, listing all ancestors
/// and weighting every function according to its importance.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Paths to .dot files with call graphs
    #[clap(short, long)]
    dot: Vec<PathBuf>,

    /// Path to file with function names to be extracted
    #[clap(short = 'e', long = "extract-list")]
    extract_functions: Option<PathBuf>,

    /// Write extracted call graph in graphviz format to file
    /// Default value is "out.dot"
    #[clap(short, long, default_value = "out.dot")]
    save_extracted: PathBuf,
    
    /// Link all files in one object file
    #[clap(short, long)]
    link: bool
}

fn main() -> io::Result<()> {
    colog::init();
    let args = Args::parse();
    // Keep objects with names to save them later if needed.
    let mut objects: Vec<(PathBuf, ObjectFile)> = vec![];
    for dot in &args.dot {
        let Ok(graph) = parse(&read_to_string(dot)?) else {
            panic!("Failed to parse .dot graph: {dot:?}");
        };
        let mut output_path = dot.clone();
        output_path.set_extension("out.dot");
        objects.push((
            output_path, 
            ObjectFile::from(TypedGraph::from(graph))
        ));
    }
    
    // Link if needed
    if args.link {
        let linked = objects.into_iter()
            .map(|p| p.1)
            .reduce(ObjectFile::link_consuming)
            .unwrap();
        objects = vec![(args.save_extracted.clone(), linked)];
    }

    for (save_to, obj_file) in objects {
        let dot_graph = Graph::from(TypedGraph::from(obj_file));
        let mut ctx = PrinterContext::default();
        let _ = fs::write(save_to, dot_graph.print(&mut ctx)).inspect_err(|err| {
            warn!("Failed to write .dot file: {err}");
        });
    }
    Ok(())
}
