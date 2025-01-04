use clap::Parser;
use graphviz_rust::parse;
use graphviz_rust::printer::{DotPrinter, PrinterContext};
use std::fs::read_to_string;
use std::path::PathBuf;
use std::{fs, io};
use std::collections::HashSet;
use graphviz_rust::dot_structures::{Graph, Id};
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
    
    #[clap(long)]
    no_inv: bool,

    /// Path to file with function names to be extracted.
    /// With --no-inv flag program will construct call graph with
    /// functions reachable from listed in file. 
    /// Otherwise, graph will contain functions, from which any listed one
    /// can be reached(inverse call graph).
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

fn mark_reachable_functions(extract_list: PathBuf, objects: &mut [(PathBuf, TypedGraph<Id>)]) -> io::Result<()> {
    let tags = read_to_string(extract_list)?
        .lines()
        .map(|l| l.trim().to_string())
        .collect::<HashSet<_>>();
    for (_, graph) in objects.iter_mut() {
        let tagged_nodes = graph.mapping()
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                if tags.contains(get_id_str(node)) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let reachable = graph.get_reachable(&tagged_nodes);
        *graph = graph.projection(&reachable).0
    }
    Ok(())
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
    
    // Convert ObjectFile to TypedGraph
    let mut typed_graphs = objects.into_iter()
        .map(|p| (p.0, TypedGraph::from(p.1)))
        .collect::<Vec<_>>();
    
    // Invert graph for reachable purposes
    if !args.no_inv {
        typed_graphs.iter_mut().for_each(|(_, graph)| *graph = graph.inv())
    }
    
    // Extract subgraph
    if let Some(extracted) = args.extract_functions {
        mark_reachable_functions(extracted, &mut typed_graphs)?;
    }
    
    // Invert graph back
    if !args.no_inv {
        typed_graphs.iter_mut().for_each(|(_, graph)| *graph = graph.inv())
    }

    for (save_to, gr) in typed_graphs {
        let dot_graph = Graph::from(gr);
        let mut ctx = PrinterContext::default();
        let _ = fs::write(save_to, dot_graph.print(&mut ctx)).inspect_err(|err| {
            warn!("Failed to write .dot file: {err}");
        });
    }
    Ok(())
}
