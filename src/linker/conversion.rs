use std::collections::HashMap;
use graphviz_rust::dot_structures::{EdgeTy, Id, Stmt};
use graphviz_rust::dot_structures::Vertex::N;
use log::warn;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use crate::linker::object_file::{ObjectFile, SymPtr};
use crate::linker::symbol::{FCall, Function};
use crate::linker::object_file::SymPtr::F;

type DotGraph = graphviz_rust::dot_structures::Graph;

fn get_id_str(id: &Id) -> &str {
    match id {
        Id::Html(s) => s,
        Id::Escaped(s) => &s[1..s.len() - 1], // fix for quoted names
        Id::Plain(s) => s,
        Id::Anonymous(s) => s,
    }
}

impl From<DotGraph> for ObjectFile {
    fn from(value: DotGraph) -> Self {
        let dot_graph = match value {
            DotGraph::Graph { stmts, .. } => stmts,
            DotGraph::DiGraph { stmts, .. } => stmts
        };
        let mut obj_file = ObjectFile::new();
        let mut node_id_to_v = HashMap::<String, SymPtr>::new();

        let ensure_node =
            |id: &Id, obj: &mut ObjectFile, mapping: &mut HashMap<String, SymPtr>| {
                mapping
                    .entry(get_id_str(id).to_string())
                    .or_insert_with(|| 
                        obj.add_function(
                            Function::new(get_id_str(id).to_string(), false)
                        ).0
                    );
            };

        for stmt in dot_graph {
            match stmt {
                Stmt::Node(node) => {
                    ensure_node(&node.id.0, &mut obj_file, &mut node_id_to_v);
                }
                Stmt::Edge(edge) => match &edge.ty {
                    EdgeTy::Pair(from, to) => match &(from, to) {
                        (N(v), N(u)) => {
                            ensure_node(&v.0, &mut obj_file, &mut node_id_to_v);
                            ensure_node(&u.0, &mut obj_file, &mut node_id_to_v);
                            obj_file.add_fcall(
                                FCall::new_with_callsite(
                                    node_id_to_v[get_id_str(&u.0)],
                                    vec![],
                                    node_id_to_v[get_id_str(&v.0)]
                                )
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
        obj_file
    }
}

impl From<ObjectFile> for Graph<String, ()> {
    fn from(value: ObjectFile) -> Self {
        if !value.objects.is_empty() || !value.points_to.is_empty() {
            warn!(
                "ObjectFile has some objects or points-to sets.\
                 Conversion to dot graph discards this data!"
            );
        }
        let mut graph = Graph::<String, ()>::with_capacity(value.functions.len(), value.calls.len());
        for (obj_idx, f) in value.functions.iter().enumerate() {
            let id = graph.add_node(f.get_name().clone());
            assert_eq!(id.index(), obj_idx);
        }
        for call in &value.calls {
            let Some(F(callsite)) = call.callsite else {
                warn!(
                    "Call {call:?} has wrong or missing callsite, discarding it",
                );
                continue;
            };
            for f in value.get_referenced_functions(call.callee) {
                graph.add_edge(NodeIndex::new(callsite), NodeIndex::new(f), ());
            }
        }
        graph
    }
}
