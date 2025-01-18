use std::collections::HashSet;
use std::hash::Hash;
use log::{debug, info, error};
use petgraph::adj::DefaultIx;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::prelude::{Dfs, EdgeRef};
use regex::Regex;

pub trait Pass {
    fn run_pass(&self, graph: &mut Graph<String, ()>);

    fn name(&self) -> String;
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

impl Pass for TerminateNodePass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        *graph = graph.filter_map(
            |_, name| if self.terminate_funcs.contains(name) {
                Some(name.clone())
            } else {
                debug!("Terminating node {name}");
                None
            },
            |_, ()| Some(())
        );
    }

    fn name(&self) -> String {
        "node terminator".to_string()
    }
}

pub enum RegexMatchAction<T>
where T : Hash + Eq {
    AddIncoming(HashSet<T>),
    AddOutgoing(HashSet<T>),
}

impl RegexMatchAction<String> {
    fn to_idx_list(&self, graph: &Graph<String, ()>) -> RegexMatchAction<NodeIndex> {
        let required_symbols = match &self {
            RegexMatchAction::AddIncoming(l)
            | RegexMatchAction::AddOutgoing(l) => l
        };
        let matched = graph
            .node_indices()
            .filter(|&idx| required_symbols.contains(&graph[idx]))
            .collect();
        match &self {
            RegexMatchAction::AddIncoming(_) => RegexMatchAction::AddIncoming(matched),
            RegexMatchAction::AddOutgoing(_) => RegexMatchAction::AddOutgoing(matched),
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

impl Pass for RegexNodePass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        let resolved_rules: Vec<(&Regex, RegexMatchAction<NodeIndex>)> = self.rules
            .iter()
            .map(|(r, action)| (r, action.to_idx_list(graph)))
            .collect();
        let mut total_resolved: usize = 0;

        for idx in graph.node_indices() {
            for (re, links) in &resolved_rules {
                if !re.is_match(&graph[idx]) {
                    continue;
                }
                // This function matched regex
                let this_f_id = HashSet::from([idx]);
                let from_funcs: &HashSet<NodeIndex>;
                let to_funcs: &HashSet<NodeIndex>;

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
                        debug!("Adding {} -> {}", graph[src], graph[dst]);
                        graph.add_edge(src, dst, ());
                    }
                }
            }
        }
        info!("RegexNodePass resolved {} calls", total_resolved);
    }

    fn name(&self) -> String {
        "regex edge generator".to_string()
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

impl Pass for CutWidthPass {
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

    fn name(&self) -> String {
        format!(
            "degree filtering(incoming < {}, outgoing < {})",
            self.max_incoming + 1,
            self.max_outgoing + 1
        )
    }
}

#[derive(Default)]
pub struct UniqueEdgesPass {}

impl Pass for UniqueEdgesPass {
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

    fn name(&self) -> String {
        "decouple edges".to_string()
    }
}

pub struct SubgraphExtractionPass {
    tags: HashSet<String>,
}

impl SubgraphExtractionPass {
    #[must_use]
    pub fn new(tags: HashSet<String>) -> Self {
        Self { tags }
    }

    #[must_use]
    pub fn new_from_str(data: &str) -> Self {
        Self {
            tags: data.split_whitespace()
                .map(ToString::to_string)
                .collect(),
        }
    }
}

impl Pass for SubgraphExtractionPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        let tagged_nodes = graph.node_weights()
            .enumerate()
            .filter_map(|(i, node)| {
                if self.tags.contains(node) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut dfs_visitor = Dfs::empty(&*graph);
        let mut visited = HashSet::new();
        for v in tagged_nodes {
            #[allow(clippy::cast_possible_truncation)]
            dfs_visitor.move_to(NodeIndex::from(v as DefaultIx));
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
            |_, ()| Some(())
        );
    }

    fn name(&self) -> String {
        "subgraph extraction".to_string()
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