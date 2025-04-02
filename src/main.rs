use clap::Parser;
use graphviz_rust::parse;
use petgraph::dot::{Config, Dot};
use std::fs::{read_to_string, File};
use std::path::PathBuf;
use std::{fs, io};
use std::io::{BufRead, BufReader};
use log::{debug, info, warn};
use petgraph::Graph;
use inv_call_extract::linker::config::parse_config_file;
use crate::linker::conversion::graphviz_to_graph;
use crate::linker::graph_link::link_all_graphs;

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
    
    #[clap(short, long)]
    config: PathBuf,

    /// Write extracted call graph in graphviz format to file
    /// Default value is "out.dot"
    #[clap(short, long, default_value = "out.dot")]
    save_extracted: PathBuf,
}

fn run_passes(args: &Args, objects: &mut Vec<(PathBuf, Graph<String, ()>)>) -> io::Result<()> {
    let (before_link, should_link, after_link) = parse_config_file(&args.config)?;
    for pass in before_link {
        info!("Running pass before link: {}", pass.name());
        objects.iter_mut()
            .for_each(|(_, graph)| pass.run_pass(graph));
    }
    if should_link {
        let linked = link_all_graphs(
            &objects.iter().map(|p| p.1.clone()).collect::<Vec<_>>()
        );
        *objects = vec![(args.save_extracted.clone(), linked)];
        info!("Linked graphs");
    }
    for pass in after_link {
        info!("Running pass after link: {}", pass.name());
        objects.iter_mut()
            .for_each(|(_, graph)| pass.run_pass(graph));
    }

    Ok(())
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
        debug!("reading {dot}");
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
    let args = Args::parse();
    // Keep objects with names to save them later if needed.
    let mut graphs = read_dot_graphs(&args)?;

    // Run deg pass on extracted subgraph
    run_passes(&args, &mut graphs)?;

    for (save_to, gr) in graphs {
        let dot_graph = Dot::with_config(&gr, &[Config::EdgeNoLabel]);
        let _ = fs::write(save_to, format!("{dot_graph:?}")).inspect_err(|err| {
            warn!("Failed to write .dot file: {err}");
        });
    }
    Ok(())
}
