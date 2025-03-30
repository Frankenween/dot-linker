use std::{fs, io};
use std::path::PathBuf;
use log::error;
use crate::linker::pass::{CutDegPass, Pass, RegexEdgeGenPass, RemoveEdgesPass, ReparentGraphPass, ReverseGraphPass, SubgraphExtractionPass, TerminateNodePass, UniqueEdgesPass};

fn parse_line(config_line: &str, line_number: usize) -> io::Result<Box<dyn Pass>> {
    let line = config_line
        .split_whitespace()
        .collect::<Vec<&str>>();
    let pass = line[0];
    match pass {
        "term_nodes" => {
            let data = fs::read_to_string(
                line.get(1).ok_or(io::ErrorKind::UnexpectedEof)?
            )?;
            Ok(Box::new(TerminateNodePass::new_from_str(&data)))
        },
        "regex_edge_gen" => {
            let data = fs::read_to_string(
                line.get(1).ok_or(io::ErrorKind::UnexpectedEof)?
            )?;
            Ok(Box::new(RegexEdgeGenPass::new_from_str(&data)))
        },
        "cut_deg" => {
            // TODO: ensure proper argument parsing
            let mut incoming: Option<usize> = None;
            let mut outgoing: Option<usize> = None;
            for arg in &line[1..] {
                let sign = arg.chars().next().unwrap();
                match sign {
                    '+' => incoming = Some(
                        arg[1..]
                            .parse::<usize>()
                            .map_err(|_| io::ErrorKind::InvalidInput)?
                    ),
                    '-' => outgoing = Some(
                        arg[1..]
                            .parse::<usize>()
                            .map_err(|_| io::ErrorKind::InvalidInput)?
                    ),
                    _ => {
                        error!("Invalid prefix for deg filter on line {line_number}.\
                         Expected '+' or '-', got {}", sign
                        );
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                }
            }
            Ok(Box::new(CutDegPass::new(incoming, outgoing)))
        },
        "unique_edges" => {
            Ok(Box::new(UniqueEdgesPass::default()))
        },
        "extract_subgraph" => {
            let data = fs::read_to_string(
                line.get(1).ok_or(io::ErrorKind::UnexpectedEof)?
            )?;
            Ok(Box::new(SubgraphExtractionPass::new_from_str(&data)))
        },
        "reverse" => {
            Ok(Box::new(ReverseGraphPass::default()))
        },
        "reparent" => {
            let data = fs::read_to_string(
                line.get(1).ok_or(io::ErrorKind::UnexpectedEof)?
            )?;
            Ok(Box::new(ReparentGraphPass::new_from_str(&data)))
        },
        "remove_edges" => {
            let data = fs::read_to_string(
                line.get(1).ok_or(io::ErrorKind::UnexpectedEof)?
            )?;
            Ok(Box::new(RemoveEdgesPass::new_from_str(&data)))
        },
        _ => {
            error!("Invalid config on line {line_number}: no \"{pass}\" pass");
            Err(io::ErrorKind::InvalidInput.into())
        }
    }
}

pub fn parse_config_file(config_file: &PathBuf) 
    -> io::Result<Vec<Box<dyn Pass>>> {
    let config_file_contents = fs::read_to_string(config_file)?;
    let mut passes: Vec<Box<dyn Pass>> = vec![];
    
    for (line_number, line) in config_file_contents.lines().enumerate() {
        passes.push(parse_line(line, line_number)?);
    }
    Ok(passes)
}