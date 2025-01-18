use clap::Parser;
use graphviz_rust::parse;
use petgraph::graph::{DefaultIx, DiGraph, NodeIndex};
use petgraph::dot::{Config, Dot};
use petgraph::visit::Dfs;
use std::fs::{read_to_string, File};
use std::path::PathBuf;
use std::{fs, io};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::mem::swap;
use log::{info, warn};
use petgraph::Graph;
use crate::linker::conversion::graphviz_to_graph;
use crate::linker::graph_link;
use crate::linker::pass::{CutWidthPass, SubgraphExtractionPass, UniqueEdgesPass};
use crate::linker::pass::Pass;

pub mod linker;

/// Program that builds inverse call graph with required functions only.
/// It can be used for creating new .dot graph, listing all ancestors
/// and weighting every function according to its importance.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// File with list of dot files to process.
    /// If not provided, paths to dot files are read from stdin
    #[clap(short, long)]
    dots: Option<PathBuf>,
    
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
    pass_max_outgoing: Option<usize>,

    /// Make new calls using regex rules from provided file.
    /// Each rule is described in format `"regex" (->|<-) name1 name2 ...`
    #[clap(long)]
    pass_node_regex: Option<PathBuf>,

    /// Do not remove multiple edges
    /// This may affect passes that remove nodes based on their degrees
    #[clap(long)]
    allow_duplicate_calls: bool,
}

fn mark_reachable_functions(extract_list: PathBuf, objects: &mut [(PathBuf, DiGraph<String, ()>)]) -> io::Result<()> {
    let tags = read_to_string(extract_list)?;
    let extractor = SubgraphExtractionPass::new_from_str(&tags);
    for (_, graph) in objects.iter_mut() {
        extractor.run_pass(graph);
    }
    Ok(())
}

fn run_graph_passes(args: &Args, objects: &mut [(PathBuf, Graph<String, ()>)]) {
    let mut passes: Vec<Box<dyn Pass>> = vec![];

    if !args.allow_duplicate_calls {
        passes.push(Box::new(UniqueEdgesPass::default()));
    }
    passes.push(Box::new(CutWidthPass::new(args.pass_max_incoming, args.pass_max_outgoing)));

    for pass in passes {
        info!("Running pass {}", pass.name());
        objects.iter_mut()
            .for_each(|(_, graph)| pass.run_pass(graph));
    }
}

fn read_dot_graphs(args: &Args) -> io::Result<Vec<(PathBuf, Graph<String, ()>)>> {
    let mut objects: Vec<(PathBuf, Graph<String, ()>)> = vec![];
    let files = match &args.dots {
        None => {
            BufReader::new(io::stdin())
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<_>>()
        },
        Some(dots) => {
            BufReader::new(File::open(dots)?)
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<_>>()
        }
    };
    for dot in &files {
        let path = PathBuf::from(dot);
        let Ok(graph) = parse(&read_to_string(path.clone())?) else {
            panic!("Failed to parse .dot graph: {dot:?}");
        };
        let mut output_path = path;
        output_path.set_extension("out.dot");
        objects.push((
            output_path,
            graphviz_to_graph(&graph)
        ));
    }
    Ok(objects)

}

fn main() -> io::Result<()> {
    colog::init();
    let mut args = Args::parse();
    // Keep objects with names to save them later if needed.
    let mut objects = read_dot_graphs(&args)?;
    // Link if needed
    if args.link {
        let linked = objects.into_iter()
            .map(|p| p.1)
            .reduce(graph_link::link_graphs)
            .unwrap();
        objects = vec![(args.save_extracted.clone(), linked)];
    }
    
    // Convert ObjectFile to TypedGraph
    let mut typed_graphs = objects.into_iter()
        .map(|p| (p.0, DiGraph::from(p.1)))
        .collect::<Vec<_>>();
    
    // Invert graph for reachable purposes
    if !args.no_inv {
        typed_graphs.iter_mut().for_each(|(_, graph)| graph.reverse());
        // In reversed graph incoming edges become outgoing and vice versa
        swap(&mut args.pass_max_incoming, &mut args.pass_max_outgoing);
    }
    
    // Extract subgraph
    if let Some(extracted) = &args.extract_functions {
        mark_reachable_functions(extracted.clone(), &mut typed_graphs)?;
    }

    // Run deg pass on extracted subgraph
    run_graph_passes(&args, &mut typed_graphs);
    
    // After graph passes some nodes may become unreachable. Abandon them
    if let Some(extracted) = &args.extract_functions {
        mark_reachable_functions(extracted.clone(), &mut typed_graphs)?;
    }

    // Invert graph back
    if !args.no_inv {
        typed_graphs.iter_mut().for_each(|(_, graph)| graph.reverse());
    }

    for (save_to, gr) in typed_graphs {
        let dot_graph = Dot::with_config(&gr, &[Config::EdgeNoLabel]);
        let _ = fs::write(save_to, format!("{dot_graph:?}")).inspect_err(|err| {
            warn!("Failed to write .dot file: {err}");
        });
    }
    Ok(())
}
