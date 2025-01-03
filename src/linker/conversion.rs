use graphviz_rust::dot_structures::{Graph, Id};
use log::{info, warn};
use crate::linker::object_file::ObjectFile;
use crate::linker::symbol::{FCall, Function};
use crate::{get_id_str, TypedGraph};
use crate::linker::object_file::SymPtr::F;

impl From<TypedGraph<Id>> for ObjectFile {
    fn from(value: TypedGraph<Id>) -> Self {
        let mut obj = Self::new();
        for id in value.mapping() {
            // Here the sequential numbering is used
            obj.add_function(
                Function::new(get_id_str(id).to_string(), false)
            );
        }
        for v in 0..value.size() {
            for u in value.next(v) {
                obj.add_fcall(
                    FCall::new_with_callsite(
                        F(*u), vec![], F(v)
                    )
                );
            }
        }
        obj
    }
}

impl From<ObjectFile> for TypedGraph<Id> {
    fn from(value: ObjectFile) -> Self {
        if !value.objects.is_empty() || !value.points_to.is_empty() {
            warn!(
                "ObjectFile has some objects or points-to sets.\
                 Conversion to dot graph discards this data!"
            );
        }
        let mut graph = TypedGraph::new_with_mapping(value.functions.clone());
        for call in &value.calls {
            let Some(F(callsite)) = call.callsite else {
                warn!(
                    "Call {call:?} has wrong or missing callsite, discarding it",
                );
                continue;
            };
            for f in value.get_referenced_functions(call.callee) {
                graph.add_edge(callsite, f);
            }
        }
        graph.map(
            |f| Id::Plain(f.get_name().clone())
        )
    }
}
