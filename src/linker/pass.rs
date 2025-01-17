use std::collections::HashSet;
use log::{debug, info, error};
use petgraph::Graph;
use petgraph::prelude::EdgeRef;
use regex::Regex;
use crate::linker::object_file::{ObjectFile, SymPtr};
use crate::linker::symbol::FCall;

pub trait LinkerPass {
    /// Run pass and modify object file
    /// NOTE: All `FCall` pointers are invalidated, pointers to functions, 
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
            terminate_funcs: iter.map(String::from).collect()
        }
    }
    
    #[must_use]
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

pub enum RegexMatchAction<T> {
    AddIncoming(Vec<T>),
    AddOutgoing(Vec<T>),
}

impl RegexMatchAction<String> {
    fn to_ptr_list(&self, obj: &ObjectFile) -> RegexMatchAction<usize> {
        match &self {
            RegexMatchAction::AddIncoming(l) => RegexMatchAction::AddIncoming(
                l.iter()
                .filter_map(|f| obj.get_fun_id(f))
                .collect()
            ),
            RegexMatchAction::AddOutgoing(l) => RegexMatchAction::AddIncoming(
                l.iter()
                    .filter_map(|f| obj.get_fun_id(f))
                    .collect()
            ),
        }
    }
}

#[derive(Default)]
pub struct RegexNodePass {
    rules: Vec<(Regex, RegexMatchAction<String>)>
}

impl RegexNodePass {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new_from_lines(data: &str) -> Self {
        let mut result = Self::new();
        for line in data.lines() {
            result.add_rule_from_line(line);
        }
        result
    }

    pub fn add_rule(&mut self, rule: (Regex, RegexMatchAction<String>)) {
        self.rules.push(rule);
    }
    
    fn split_line(line: &str) -> Option<(&str, &str, bool)> {
        if let Some((regex, list_part)) = line.split_once("->") {
            Some((regex, list_part, false))
        } else if let Some((regex, list_part)) = line.split_once("<-") {
            Some((regex, list_part, true))
        } else {
            None
        }
    }

    pub fn add_rule_from_line(&mut self, line: &str) {
        let Some((regex_part, list_part, incoming)) = Self::split_line(line) else {
            error!("Rule line does not contain '->' or '<-' separator, discarding it: \"{}\"", line);
            return;
        };
        let regex_str = regex_part.trim();
        if !regex_str.starts_with('\"') 
            || !regex_str.ends_with('\"') 
            || regex_str.len() < 2 {
            error!("Regex part is not wrapped with quotes, discarding it: \"{}\"", line);
            return;
        }
        let Ok(regex) = Regex::new(&regex_str[1..regex_str.len() - 1]) else {
            error!("Regex is incorrect, discarding it: \"{}\"", line);
            return;
        };
        let symlist = list_part.split_whitespace()
            .map(ToString::to_string)
            .collect();

        if incoming {
            self.rules.push((
                regex,
                RegexMatchAction::AddIncoming(symlist)
            ));
        } else {
            self.rules.push((
                regex,
                RegexMatchAction::AddOutgoing(symlist)
            ));
        }
    }
}

impl LinkerPass for RegexNodePass {
    fn run_pass(&self, obj: &mut ObjectFile) {
        let resolved_rules: Vec<(&Regex, RegexMatchAction<usize>)> = self.rules
            .iter()
            .map(|(r, action)| (r, action.to_ptr_list(obj)))
            .collect();
        let mut total_resolved: usize = 0;

        for (idx, function) in obj.functions.iter().enumerate() {
            for (re, links) in &resolved_rules {
                if !re.is_match(function.get_name()) {
                    continue;
                }
                // This function matched regex
                let this_f_id = vec![idx];
                let from_funcs: &Vec<usize>;
                let to_funcs: &Vec<usize>;

                match links {
                    RegexMatchAction::AddIncoming(l) => {
                        from_funcs = l;
                        to_funcs = &this_f_id;
                    }
                    RegexMatchAction::AddOutgoing(l) => {
                        from_funcs = &this_f_id;
                        to_funcs = l;
                    }
                }

                for &src in from_funcs {
                    for &dst in to_funcs {
                        total_resolved += 1;
                        debug!("Adding {} -> {}",
                                obj.functions[src].get_name(),
                                obj.functions[dst].get_name(),
                            );
                        obj.calls.push(
                            FCall::new_with_callsite(
                                SymPtr::F(dst),
                                vec![],
                                SymPtr::F(src), // callsite is last!
                            )
                        );
                    }
                }
            }
        }
        info!("RegexNodePass resolved {} calls", total_resolved);
    }
}

pub struct CutWidthPass {
    max_incoming: usize,
    max_outgoing: usize,
}

impl CutWidthPass {
    #[must_use]
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

#[derive(Default)]
pub struct UniqueEdgesPass {}

impl GraphPass for UniqueEdgesPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        let mut added_nodes: HashSet<(usize, usize)> = HashSet::new();
        *graph = graph.filter_map(
            |_, v| Some(v.clone()),
            |idx, ()| {
                let (src, dst) = graph.edge_endpoints(idx)?;
                if added_nodes.insert((src.index(), dst.index())) {
                    Some(())
                } else {
                    None
                }
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_unique_edges() {
        let mut graph = Graph::new();
        let v = [
            graph.add_node("1".to_string()),
            graph.add_node("2".to_string()),
            graph.add_node("3".to_string())
        ];
        
        // 0 -> (1, 2)
        // 1 -> (0, 2)
        // 2 -> (2, 1)
        let mut adj_matrix = vec![vec![0; 3]; 3];
        adj_matrix[0][1] = 1;
        adj_matrix[0][2] = 1;
        adj_matrix[1][0] = 1;
        adj_matrix[1][2] = 1;
        adj_matrix[2][1] = 1;
        adj_matrix[2][2] = 1;
        
        graph.add_edge(v[0], v[2], ());
        graph.add_edge(v[0], v[2], ());
        graph.add_edge(v[0], v[1], ());
        graph.add_edge(v[0], v[2], ());
        
        graph.add_edge(v[1], v[0], ());
        graph.add_edge(v[1], v[2], ());
        
        graph.add_edge(v[2], v[2], ());
        graph.add_edge(v[2], v[1], ());
        graph.add_edge(v[2], v[2], ());
        graph.add_edge(v[2], v[1], ());
        
        let pass = UniqueEdgesPass::default();
        pass.run_pass(&mut graph);
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(adj_matrix[i][j], graph.edges_connecting(v[i], v[j]).count());
            }
        }
    }
}