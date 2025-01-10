use std::collections::HashSet;
use log::debug;
use petgraph::Graph;
use petgraph::prelude::EdgeRef;
use crate::linker::object_file::ObjectFile;

pub trait LinkerPass {
    /// Run pass and modify object file
    /// NOTE: All FCall pointers are invalidated, pointers to functions, 
    /// points-to sets and globals are guaranteed to be valid, but content may change.
    fn run_pass(&self, obj: &mut ObjectFile);
}

pub trait GraphPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>);
}

/// Make all listed functions terminal, after this pass there will be no calls from them.
pub struct TerminateNodePass {
    terminate_funcs: HashSet<String>
}

impl TerminateNodePass {
    pub fn new(iter: &mut dyn Iterator<Item = &str>) -> Self {
        Self {
            terminate_funcs: HashSet::from_iter(iter.map(String::from))
        }
    }
    
    pub fn new_from_str(s: &str) -> Self {
        Self::new(&mut s.split_whitespace())
    }
}

impl LinkerPass for TerminateNodePass {
    fn run_pass(&self, obj: &mut ObjectFile) {
        for i in (0..obj.calls.len()).rev() {
            let Some(callsite_id) = &obj.calls[i].callsite else {
                continue;
            };
            let callsite = obj.get_fun_by_id(callsite_id);
            if self.terminate_funcs.contains(callsite.get_name()) {
                debug!("Removing {} -> * call", callsite.get_name());
                // We processed the calls after us so it's safe to swap-remove it
                obj.calls.swap_remove(i);
            }
        }
    }
}

pub struct CutWidthPass {
    max_incoming: usize,
    max_outgoing: usize,
}

impl CutWidthPass {
    pub fn new(max_incoming: Option<usize>, max_outgoing: Option<usize>) -> Self {
        Self {
            max_incoming: max_incoming.unwrap_or(usize::MAX),
            max_outgoing: max_outgoing.unwrap_or(usize::MAX),
        }
    }
}

impl GraphPass for CutWidthPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        // (deg-in; deg-out)
        let mut deg: Vec<(usize, usize)> = vec![(0, 0); graph.node_count()];
        for edge in graph.edge_references() {
            deg[edge.source().index()].1 += 1;
            deg[edge.target().index()].0 += 1;
        }
        let keep_nodes = deg
            .iter()
            .enumerate()
            .filter_map(|(v, &(deg_in, deg_out))|
                if deg_in <= self.max_incoming && deg_out <= self.max_outgoing {
                    Some(v)
                } else {
                    None
                }
            )
            .collect::<HashSet<_>>();
        graph.retain_nodes(|_, v| keep_nodes.contains(&v.index()));
    }
}