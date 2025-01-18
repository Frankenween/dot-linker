use std::collections::HashMap;
use std::hash::Hash;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::prelude::EdgeRef;

#[must_use] 
pub fn link_graphs<N, E>(g1: Graph<N, E>, g2: Graph<N, E>) -> Graph<N, E> 
where N: Clone + Hash + Eq, E: Clone {
    let mut result = g1.clone();
    let mut mapping: HashMap<&N, NodeIndex> = HashMap::new();
    // Nodes from g1 already exist
    for idx in result.node_indices() {
        mapping.insert(&g1[idx], idx);
    }
    // Nodes from g2 should be added
    for idx in g2.node_indices() {
        if !mapping.contains_key(&g2[idx]) {
            mapping.insert(&g2[idx], result.add_node(g2[idx].clone()));
        }
    }
    for edge in g2.edge_references() {
        result.add_edge(
            mapping[&g2[edge.source()]], 
            mapping[&g2[edge.target()]], 
            edge.weight().clone()
        );
    }
    result
}