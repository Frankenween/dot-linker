use std::collections::HashSet;
use std::hash::Hash;
use log::{debug, info, error};
use petgraph::adj::DefaultIx;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::prelude::{Dfs, EdgeRef};
use fancy_regex::Regex;

pub trait Pass {
    fn run_pass(&self, graph: &mut Graph<String, ()>);

    fn name(&self) -> String;
}

/// Make all listed functions terminal, after this pass there will be no such nodes.
pub struct RemoveNodePass {
    terminate_funcs: Vec<Regex>
}

impl RemoveNodePass {
    pub fn new(iter: &mut dyn Iterator<Item = &str>) -> Self {
        Self {
            terminate_funcs: iter.map(|s| Regex::new(s).unwrap()).collect()
        }
    }

    #[must_use]
    pub fn new_from_str(s: &str) -> Self {
        Self::new(&mut s.split_whitespace())
    }
}

impl Pass for RemoveNodePass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        *graph = graph.filter_map(
            |_, name| if self.terminate_funcs
                .iter()
                .any(|re| re.is_match(name).unwrap()) {
                debug!("Terminating node {name}");
                None
            } else {
                Some(name.clone())
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
pub struct RegexEdgeGenPass {
    rules: Vec<(Regex, RegexMatchAction<String>)>
}

impl RegexEdgeGenPass {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new_from_str(data: &str) -> Self {
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

impl Pass for RegexEdgeGenPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        let resolved_rules: Vec<(&Regex, RegexMatchAction<NodeIndex>)> = self.rules
            .iter()
            .map(|(r, action)| (r, action.to_idx_list(graph)))
            .collect();
        let mut total_resolved: usize = 0;

        for idx in graph.node_indices() {
            for (re, links) in &resolved_rules {
                if !re.is_match(&graph[idx]).unwrap() {
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

pub struct CutDegPass {
    max_incoming: usize,
    max_outgoing: usize,
}

impl CutDegPass {
    #[must_use]
    pub fn new(max_incoming: Option<usize>, max_outgoing: Option<usize>) -> Self {
        Self {
            max_incoming: max_incoming.unwrap_or(usize::MAX),
            max_outgoing: max_outgoing.unwrap_or(usize::MAX),
        }
    }
}

impl Pass for CutDegPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        // (deg-in; deg-out)
        let mut deg: Vec<(usize, usize)> = vec![(0, 0); graph.node_count()];
        for edge in graph.edge_references() {
            deg[edge.source().index()].1 += 1;
            deg[edge.target().index()].0 += 1;
        }
        graph.retain_nodes(
            |_, v| deg[v.index()].0 <= self.max_incoming &&
                deg[v.index()].1 <= self.max_outgoing,
        );
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

#[derive(Default)]
pub struct ReverseGraphPass {}

impl Pass for ReverseGraphPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        graph.reverse();
    }

    fn name(&self) -> String {
        "reverse graph".to_string()
    }
}

// Reparent all children of matching nodes
// If we have v -> matched -> u, then an edge v -> u is added
// All nodes are preserved
pub struct ReparentGraphPass {
    reparent_rules: Vec<Regex>
}

impl ReparentGraphPass {
    #[must_use]
    pub fn new_from_str(data: &str) -> Self {
        Self {
            reparent_rules: data.lines()
                .flat_map(|l| {
                    Regex::new(l).inspect_err(|e| error!("Wrong regex \"{}\": {}", l, e))
                })
                .collect(),
        }
    }
}

impl Pass for ReparentGraphPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        let mut new_graph = graph.clone();
        let mut matched_nodes = HashSet::new();
        let mut reparanted = 0usize;
        for node in graph.node_indices() {
            if self.reparent_rules.iter()
                .any(|rule| rule.is_match(&graph[node]).unwrap()) {
                matched_nodes.insert(node);
            }
        }
        for v in graph.node_indices() {
            for next in graph
                .neighbors(v)
                .filter(|n| matched_nodes.contains(n)) {
                // need to reparent all next children
                debug!("Reparent {} children to {}", next.index(), v.index());
                for child in graph.neighbors(next) {
                    new_graph.add_edge(v, child, ());
                    reparanted += 1;
                }
            }
        }
        info!(
            "Reparent pass matched {} nodes and added {} new edges", 
            matched_nodes.len(), reparanted
        );
        *graph = new_graph;
    }

    fn name(&self) -> String {
        "reparent nodes".to_string()
    }
}

#[derive(Default)]
pub struct RemoveEdgesPass {
    /// List of regular expressions in format (from_re\0to_re)
    rules: Vec<Regex>,
}

impl RemoveEdgesPass {
    pub fn new_from_str(data: &str) -> Self {
        let mut result = RemoveEdgesPass { rules: Vec::new() };
        for line in data.lines() {
            result.add_rule_from_str(line);
        }
        result
    }

    pub fn add_rule_from_str(&mut self, rule: &str) {
        let (l, r) = rule.split_once(' ').unwrap();
        self.rules.push(
            Regex::new(&Self::get_edge_string(l, r)).unwrap()
        );
    }

    fn edge_matches(&self, from_label: &str, to_label: &str) -> bool {
        self.rules.iter().any(|re| {
            re.is_match(&Self::get_edge_string(from_label, to_label)).unwrap()
        })
    }

    fn get_edge_string(from_label: &str, to_label: &str) -> String {
        from_label.to_string() + "\0" + to_label
    }
}

impl Pass for RemoveEdgesPass {
    fn run_pass(&self, graph: &mut Graph<String, ()>) {
        *graph = graph.filter_map(
            |_, name| Some(name.clone()),
            |e_idx, ()| {
                let (from, to) = graph.edge_endpoints(e_idx)?;
                if self.edge_matches(graph[from].as_ref(), graph[to].as_ref()) {
                    None
                } else {
                    Some(())
                }
            }
        );
    }

    fn name(&self) -> String {
        "remove edges".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_nodes() {
        let mut graph: Graph<String, ()> = Graph::new();
        graph.add_node("aba".to_string());
        graph.add_node("abc".to_string());
        graph.add_node("123".to_string());
        graph.add_node("xy1".to_string());

        let pass = RemoveNodePass::new_from_str("^\\d+$ (\\w).\\1");
        pass.run_pass(&mut graph);

        assert_eq!(
            graph.node_weights().collect::<HashSet<_>>(),
            ["abc".to_string(), "xy1".to_string()].iter().collect::<HashSet<_>>()
        );
    }

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

    #[test]
    fn test_reparent() {
        let mut graph: Graph<String, ()> = Graph::new();
        let v = [
            graph.add_node("0".to_string()),
            graph.add_node("1".to_string()),
            graph.add_node("reparent1".to_string()),
            graph.add_node("reparent2".to_string()),
            graph.add_node("4".to_string()),
        ];
        macro_rules! add_edge {
            ($v : expr, $u : expr) => {
                graph.add_edge(v[$v], v[$u], ())
            };
        }
        add_edge!(0, 1);
        add_edge!(0, 2);
        add_edge!(0, 3);
        add_edge!(2, 4);
        add_edge!(3, 1);
        add_edge!(3, 2);

        let mut orig_graph = graph.clone();

        let pass = ReparentGraphPass::new_from_str("reparent.*");
        pass.run_pass(&mut graph);

        // From reparent1
        orig_graph.add_edge(v[0], v[4], ());
        orig_graph.add_edge(v[3], v[4], ());
        // From reparent2
        orig_graph.add_edge(v[0], v[1], ());
        orig_graph.add_edge(v[0], v[2], ());

        for node in v {
            let mut n1 = orig_graph.edges(node)
                .map(|e| (e.source(), e.target()))
                .collect::<Vec<_>>();
            let mut n2 = graph.edges(node)
                .map(|e| (e.source(), e.target()))
                .collect::<Vec<_>>();
            n1.sort();
            n2.sort();
            assert_eq!(n1, n2);
        }
    }

    #[test]
    fn test_remove_edges() {
        let mut graph = Graph::new();
        let v = [
            graph.add_node("a_1".to_string()),
            graph.add_node("a_2".to_string()),
            graph.add_node("b_2".to_string()),
            graph.add_node("x".to_string()),
            graph.add_node("y".to_string()),
        ];
        for &i in &v {
            graph.add_edge(v[0], i, ());
        }
        let mut pass = RemoveEdgesPass::default();
        pass.add_rule_from_str("a_(.*) b.*");
        pass.add_rule_from_str(r"a_(.*) a_(?!\1)");
        pass.add_rule_from_str("^a.* [a-x]$");

        pass.run_pass(&mut graph);

        // need a_1 -> a_1, a_1 -> x
        assert_eq!(
            graph.neighbors(v[0]).map(|e| graph[e].as_ref()).collect::<HashSet<_>>(),
            HashSet::from(["a_1", "y"])
        );
    }
}