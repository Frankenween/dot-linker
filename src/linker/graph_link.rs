use std::collections::HashMap;
use std::hash::Hash;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::prelude::EdgeRef;

#[must_use]
pub fn link_all_graphs<N, E>(graphs: &[Graph<N, E>]) -> Graph<N, E>
where N: Clone + Hash + Eq, E: Clone {
    let mut result = Graph::<N, E>::new();
    let mut mapping: HashMap<&N, NodeIndex> = HashMap::new();
    for g in graphs {
        for v in g.node_weights() {
            if !mapping.contains_key(v) {
                mapping.insert(v, result.add_node(v.clone()));
            }
        }
        for edge in g.edge_references() {
            result.add_edge(
                mapping[&g[edge.source()]],
                mapping[&g[edge.target()]],
                edge.weight().clone()
            );
        }
    }
    result
}