use clap::Parser;
use graphviz_rust::parse;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::dot::{Config, Dot};
use petgraph::visit::Dfs;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::{fs, io};
use std::collections::HashSet;
use std::mem::swap;
use log::warn;
use petgraph::Graph;
use crate::linker::pass::CutWidthPass;
use crate::linker::object_file::ObjectFile;
use crate::linker::pass::{GraphPass, LinkerPass, TerminateNodePass};

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
    link: bool,
    
    /// List of functions to be terminated, so there will be no calls from them
    #[clap(long)]
    pass_term_nodes: Option<PathBuf>,
    
    /// Limit number of calls to the function in original graph
    /// If function is called more than this number, it is discarded
    #[clap(long)]
    pass_max_incoming: Option<usize>,

    /// Limit number of calls from the function in original graph
    /// If function calls more than this number of other functions, it is discarded
    #[clap(long)]
    pass_max_outgoing: Option<usize>
}

fn mark_reachable_functions(extract_list: PathBuf, objects: &mut [(PathBuf, DiGraph<String, ()>)]) -> io::Result<()> {
    let tags = read_to_string(extract_list)?
        .lines()
        .map(|l| l.trim().to_string())
        .collect::<HashSet<_>>();
    for (_, graph) in objects.iter_mut() {
        let tagged_nodes = graph.node_weights()
            .enumerate()
            .filter_map(|(i, node)| {
                if tags.contains(node) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut dfs_visitor = Dfs::empty(&*graph);
        let mut visited = HashSet::new();
        for v in tagged_nodes {
            dfs_visitor.move_to(NodeIndex::from(v as u32));
            while let Some(reached) = dfs_visitor.next(&*graph) {
                visited.insert(reached);
            }
        }
        *graph = graph.filter_map(
            |idx, value| {
                if visited.contains(&idx) { 
                    Some(value.clone()) 
                } else { 
                    None 
                }
            },
            |_, _| Some(())
        );
    }
    Ok(())
}

fn run_linker_passes(args: &Args, objects: &mut [(PathBuf, ObjectFile)]) -> io::Result<()> {
    let mut passes: Vec<Box<dyn LinkerPass>> = vec![];
    if let Some(term_nodes) = &args.pass_term_nodes {
        let data = read_to_string(term_nodes)?;
        passes.push(Box::new(TerminateNodePass::new_from_str(&data)));
    }
    for pass in passes {
        objects.iter_mut()
            .for_each(|(_, graph)| pass.run_pass(graph))
    }
    Ok(())
}

fn run_graph_passes(args: &Args, objects: &mut [(PathBuf, Graph<String, ()>)]) -> io::Result<()> {
    let mut passes: Vec<Box<dyn GraphPass>> = vec![];
    passes.push(Box::new(CutWidthPass::new(args.pass_max_incoming, args.pass_max_outgoing)));
    for pass in passes {
        objects.iter_mut()
            .for_each(|(_, graph)| pass.run_pass(graph))
    }
    Ok(())
}

fn main() -> io::Result<()> {
    colog::init();
    let mut args = Args::parse();
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
            ObjectFile::from(graph)
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
    
    run_linker_passes(&args, &mut objects)?;
    
    // Convert ObjectFile to TypedGraph
    let mut typed_graphs = objects.into_iter()
        .map(|p| (p.0, DiGraph::from(p.1)))
        .collect::<Vec<_>>();
    
    // Invert graph for reachable purposes
    if !args.no_inv {
        typed_graphs.iter_mut().for_each(|(_, graph)| graph.reverse());
        // In reversed graph incoming edges become outgoing and vice versa
        swap(&mut args.pass_max_incoming, &mut args.pass_max_outgoing)
    }
    
    // Extract subgraph
    if let Some(extracted) = &args.extract_functions {
        mark_reachable_functions(extracted.clone(), &mut typed_graphs)?;
    }

    // Run deg pass on extracted subgraph
    run_graph_passes(&args, &mut typed_graphs)?;
    
    // After graph passes some nodes may become unreachable. Abandon them
    if let Some(extracted) = &args.extract_functions {
        mark_reachable_functions(extracted.clone(), &mut typed_graphs)?;
    }

    // Invert graph back
    if !args.no_inv {
        typed_graphs.iter_mut().for_each(|(_, graph)| graph.reverse())
    }

    for (save_to, gr) in typed_graphs {
        let dot_graph = Dot::with_config(&gr, &[Config::EdgeNoLabel]);
        let _ = fs::write(save_to, format!("{:?}", dot_graph)).inspect_err(|err| {
            warn!("Failed to write .dot file: {err}");
        });
    }
    Ok(())
}
