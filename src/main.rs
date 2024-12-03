use clap::Parser;
use graphviz_rust::parse;
use graphviz_rust::printer::{DotPrinter, PrinterContext};
use inv_call_extract::{dot_to_graph, get_id_str, get_stmt_ref, mygraph_to_graphviz, node_from_id};
use std::collections::HashSet;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::{fs, io};

/// Program that builds inverse call graph with required functions only.
/// It can be used for creating new .dot graph, listing all ancestors
/// and weighting every function according to its importance.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Path to .dot file with call graph
    input: PathBuf,

    /// Path to file with function names
    functions: PathBuf,

    /// Write extracted call graph in graphviz format to file
    #[clap(short, long)]
    save_extracted: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let call_graph = match parse(&read_to_string(args.input)?) {
        Ok(g) => g,
        Err(e) => panic!("Failed to parse .dot graph: {e}"),
    };
    let tags = read_to_string(args.functions)?
        .lines()
        .map(|l| l.trim().to_string())
        .collect::<HashSet<_>>();

    let (graph, node_mapping) = dot_to_graph(get_stmt_ref(&call_graph));

    let tagged_nodes = node_mapping
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

    let inv_graph = graph.inv();
    // Here subgraph node v maps to DotId via node_mapping[proj_mapping[v]]
    let (subgraph, proj_mapping) = inv_graph.projection(&inv_graph.get_reachable(&tagged_nodes));

    if let Some(save_extracted) = args.save_extracted {
        let dot_g = mygraph_to_graphviz(
            &subgraph,
            &|v| node_from_id(&node_mapping[proj_mapping[v]]),
            "Extracted call graph",
        );
        let mut ctx = PrinterContext::default();
        fs::write(save_extracted, dot_g.print(&mut ctx))?;
    }
    Ok(())
}
