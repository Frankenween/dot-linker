use graphviz_rust::dot_generator::*;
use graphviz_rust::dot_structures::Vertex::N;
use graphviz_rust::dot_structures::*;
use std::collections::HashMap;
use crate::TypedGraph;

type MyNodeId = crate::NodeId;

pub fn get_id_str(id: &Id) -> &str {
    match id {
        Id::Html(s) => s,
        Id::Escaped(s) => &s[1..s.len() - 1], // fix for quoted names
        Id::Plain(s) => s,
        Id::Anonymous(s) => s,
    }
}

impl From<Graph> for TypedGraph<Id> {
    fn from(value: Graph) -> Self {
        let dot_graph = match value {
            Graph::Graph { stmts, .. } => stmts,
            Graph::DiGraph { stmts, .. } => stmts
        };
        let mut graph = TypedGraph::new();
        let mut node_id_to_v = HashMap::<String, MyNodeId>::new();

        let ensure_node =
            |id: &Id, graph: &mut TypedGraph<Id>, mapping: &mut HashMap<String, MyNodeId>| {
                mapping
                    .entry(get_id_str(id).to_string())
                    .or_insert_with(|| graph.new_node(id.clone()));
            };

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
}

impl From<TypedGraph<Node>> for Graph {
    fn from(value: TypedGraph<Node>) -> Self {
        let mut graph = graph!(strict di id!("callgraph"));
        let mapping = value.mapping();
        for v in 0..value.size() {
            // Define current node
            graph.add_stmt(Stmt::Node(mapping[v].clone()));

            // Add all outgoing edges
            for u in value.next(v) {
                graph.add_stmt(Stmt::Edge(edge!(mapping[v].clone().id => mapping[*u].clone().id)))
            }
        }
        graph
    }
}

impl From<TypedGraph<Id>> for Graph {
    fn from(value: TypedGraph<Id>) -> Self {
        Graph::from(value.map(node_from_id))
    }
}

fn node_from_id(id: Id) -> Node {
    Node::new(NodeId(id.clone(), None), vec![])
}

#[cfg(test)]
mod tests {
    use crate::{get_id_str, NodeId, TypedGraph};
    use graphviz_rust::dot_structures::Id;
    use graphviz_rust::parse;
    use std::collections::HashSet;

    fn get_node_by_name(mapping: &[Id], name: &str) -> Option<usize> {
        mapping.iter().enumerate().find_map(|(i, id)| {
            if get_id_str(id).eq(name) {
                Some(i)
            } else {
                None
            }
        })
    }

    #[test]
    fn test_dot_to_internal() {
        let graph_str = r#"
            strict digraph test {
                a -> b;
                b -> A1;
                c [label = "c label"];
                a [label = "a label"];
                b:0 -> c [label = "port test"];
                "A1" -> b;
            }
            "#;
        let graph = parse(graph_str).unwrap();

        let my_graph = TypedGraph::from(graph);
        let mapping = &my_graph.mapping();

        let a_node = get_node_by_name(mapping, "a").unwrap();
        let b_node = get_node_by_name(mapping, "b").unwrap();
        let a1_node = get_node_by_name(mapping, "A1").unwrap();
        let c_node = get_node_by_name(mapping, "c").unwrap();

        assert_eq!(
            my_graph.next(a_node).iter().collect::<HashSet<&NodeId>>(),
            HashSet::from([&b_node]),
            "a -> b"
        );
        assert_eq!(
            my_graph.next(b_node).iter().collect::<HashSet<&NodeId>>(),
            HashSet::from([&a1_node, &c_node]),
            "b -> 1A, c"
        );
        assert_eq!(
            my_graph.next(a1_node).iter().collect::<HashSet<&NodeId>>(),
            HashSet::from([&b_node]),
            "1A -> b"
        );
        assert_eq!(
            my_graph.next(c_node).iter().collect::<HashSet<&NodeId>>(),
            HashSet::new(),
            "c -> "
        );
    }
}
