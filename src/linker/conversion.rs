use std::collections::HashMap;
use graphviz_rust::dot_structures::{EdgeTy, Id, Stmt};
use graphviz_rust::dot_structures::Vertex::N;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

type DotGraph = graphviz_rust::dot_structures::Graph;

fn get_id_str(id: &Id) -> &str {
    match id {
        Id::Html(s) | Id::Plain(s) | Id::Anonymous(s) => s,
        Id::Escaped(s) => &s[1..s.len() - 1], // fix for quoted names
    }
}

fn ensure_node<'a, 'b>(
    id: &'a Id, 
    g: &mut Graph<String, ()>, 
    mapping: &mut HashMap<&'b str, NodeIndex>
) where 'a: 'b {
    mapping
        .entry(get_id_str(id))
        .or_insert_with(||
            g.add_node(get_id_str(id).to_string())
        );
}

#[must_use]
pub fn graphviz_to_graph(value: &DotGraph) -> Graph<String, ()> {
    let dot_graph = match value {
        DotGraph::Graph { stmts, .. }
        | DotGraph::DiGraph { stmts, .. } => stmts,
    };
    let mut graph: Graph<String, ()> = Graph::new();
    let mut node_id_to_v = HashMap::<&str, NodeIndex>::new();

    for stmt in dot_graph {
        match stmt {
            Stmt::Node(node) => {
                ensure_node(&node.id.0, &mut graph, &mut node_id_to_v);
            }
            Stmt::Edge(edge) => match &edge.ty {
                EdgeTy::Pair(from, to) => match &(from, to) {
                    (N(v), N(u)) => {
                        ensure_node(&v.0, &mut graph, &mut node_id_to_v);
                        ensure_node(&u.0, &mut graph, &mut node_id_to_v);
                        graph.add_edge(
                            node_id_to_v[get_id_str(&v.0)],
                            node_id_to_v[get_id_str(&u.0)],
                            ()
                        );
                    }
                    (_, _) => {
                        panic!("Edge type mismatch");
                    }
                },
                EdgeTy::Chain(_) => {
                    panic!("Chain not supported");
                }
            },
            _ => {}
        }
    }
    graph
}
