use std::collections::HashMap;
use log::debug;
use super::symbol::{FCall, Function, Object, PointsTo};

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum SymPtr {
    /// Index in functions list
    F(usize),
    /// Index in objects list
    O(usize),
    /// Index in points-to list
    P(usize),
    /// Index in calls list
    C(usize),
}

#[derive(Default)]
pub struct ObjectFile {
    pub(super) functions: Vec<Function>,
    pub(super) func_mapping: HashMap<String, usize>,
    pub(super) objects: Vec<Object<SymPtr>>,
    pub(super) points_to: Vec<PointsTo<SymPtr>>,
    pub(super) calls: Vec<FCall<SymPtr>>
}

impl ObjectFile {
    pub fn new() -> Self {
        ObjectFile::default()
    }

    /// Create new function symbol or return the existing one
    /// If symbol exists, return true as a second value
    pub fn add_function(&mut self, f: Function) -> (SymPtr, bool) {
        if let Some(idx) = self.func_mapping.get(f.get_name()) {
            return (SymPtr::F(*idx), true);
        }
        let handle = f.get_name().clone();
        let idx = self.functions.len();
        self.functions.push(f);
        self.func_mapping.insert(handle, idx);
        (SymPtr::F(idx), false)
    }
    
    pub fn add_object(&mut self, obj: Object<SymPtr>) -> SymPtr {
        let idx = self.objects.len();
        self.objects.push(obj);
        SymPtr::O(idx)
    }

    pub fn add_points_to(&mut self, pts: PointsTo<SymPtr>) -> SymPtr {
        let idx = self.points_to.len();
        self.points_to.push(pts);
        SymPtr::P(idx)
    }

    pub fn add_fcall(&mut self, fcall: FCall<SymPtr>) -> SymPtr {
        let idx = self.calls.len();
        self.calls.push(fcall);
        SymPtr::C(idx)
    }
    
    pub fn get_fun_id(&self, name: &str) -> Option<usize> {
        self.func_mapping.get(name).copied()
    }

    pub fn get_fun_by_name(&self, name: &str) -> Option<&Function> {
        Some(&self.functions[*self.func_mapping.get(name)?])
    }
    
    pub fn get_fun_by_id(&self, id: &SymPtr) -> &Function {
        match id {
            SymPtr::F(i) => &self.functions[*i],
            _ => panic!("not an function id")
        }
    }

    pub fn get_mut_fun_by_name(&mut self, name: &str) -> Option<&mut Function> {
        Some(&mut self.functions[*self.func_mapping.get(name)?])
    }

    fn get_mut_fun_by_id(&mut self, id: &SymPtr) -> &mut Function {
        match id {
            SymPtr::F(i) => &mut self.functions[*i],
            _ => panic!("not an function id")
        }
    }

    fn is_indirect_call(&self, fid: SymPtr) -> bool {
        let SymPtr::C(call) = fid else {
            panic!("Not a call: {fid:?}");
        };
        match self.calls[call].callee {
            SymPtr::F(_) => false,
            SymPtr::P(_) => true,
            _ => panic!("Callee is not a function or pointer set")
        }
    }
    
    pub fn get_referenced_functions(&self, id: SymPtr) -> Vec<usize> {
        match id {
            SymPtr::F(idx) => vec![idx],
            SymPtr::P(idx) => {
                self.points_to[idx]
                    .points_to
                    .iter()
                    .filter_map(|ptr| match ptr {
                        SymPtr::F(fid) => Some(*fid),
                        _ => None
                    })
                    .collect()
            },
            _ => panic!("Only function and points-to set can reference functions, not {id:?}")
        }
    }
    
    /// Return new SymPtr with changed index
    /// If it is a function pointer, new index is resolved using old mapping and 
    /// updated current mapping. Otherwise, index is just advanced.
    /// 
    /// NOTE: do not change current structure when using this function!
    fn update_external_ptr(&self, ptr: &SymPtr, src: &ObjectFile) -> SymPtr {
        match ptr {
            SymPtr::F(_) => SymPtr::F(self.func_mapping[src.get_fun_by_id(ptr).get_name()]),
            SymPtr::O(i) => SymPtr::O(*i + self.objects.len()),
            SymPtr::P(i) => SymPtr::P(*i + self.points_to.len()),
            SymPtr::C(i) => SymPtr::C(*i + self.calls.len()),
        }
    }
    
    fn update_object(&self, obj: &mut Object<SymPtr>, src: &ObjectFile) {
        for link in obj.fields.iter_mut().flatten() {
            *link = self.update_external_ptr(link, src);
        }
    }

    fn update_points_to(&self, pts: &mut PointsTo<SymPtr>, src: &ObjectFile) {
        for ptr in pts.points_to.iter_mut() {
            *ptr = self.update_external_ptr(ptr, src);
        }
    }
    
    fn update_fcall(&self, fcall: &mut FCall<SymPtr>, src: &ObjectFile) {
        fcall.callee = self.update_external_ptr(&fcall.callee, src);
        for arg in fcall.arguments.iter_mut().flatten() {
            *arg = self.update_external_ptr(arg, src);
        }
        if let Some(callsite) = &mut fcall.callsite {
            *callsite = self.update_external_ptr(callsite, src);
        }
    }

    /// Link this object file with the other one.
    /// The only thing object files can share is function symbol.
    /// Current algorithm merges two function symbol sets, updating external flag if needed,
    /// and just concatenates other symbol lists, changing indexes in symbols from the other file.
    pub fn link(&mut self, mut other: ObjectFile) {
        // Dummy object to save mapping
        let mut mapper = ObjectFile::new();
        
        for f in other.functions.iter() {
            mapper.add_function(f.clone());
            let (id, existed) = self.add_function(f.clone());
            if existed && f.is_external() != self.get_fun_by_id(&id).is_external() {
                debug!("Function {} is now internal", f.get_name());
                self.get_mut_fun_by_id(&id).set_external(false);
            }
        }
        // After this point `other` contains inconsistent data!
        // Update objects in-place, then move them together
        for o in other.objects.iter_mut() {
            self.update_object(o, &mapper);
        }
        for p in other.points_to.iter_mut() {
            self.update_points_to(p, &mapper);
        }
        for fc in other.calls.iter_mut() {
            self.update_fcall(fc, &mapper);
        }
        // Now all pointers in `other` are valid in self. Move all symbols
        self.objects.append(&mut other.objects);
        self.points_to.append(&mut other.points_to);
        self.calls.append(&mut other.calls);
    }
    
    pub fn link_consuming(mut o1: Self, o2: Self) -> Self {
        o1.link(o2);
        o1
    }
}

#[cfg(test)]
mod tests {
    use crate::linker::object_file::ObjectFile;
    use crate::linker::object_file::SymPtr::{F, O, P, C};
    use crate::linker::symbol::{FCall, Function, Object, PointsTo};

    #[test]
    fn test_func_add() {
        let mut obj = ObjectFile::new();
        assert_eq!(
            obj.add_function(Function::new("f1".to_string(), true)),
            (F(0), false)
        );
        assert_eq!(
            obj.add_function(Function::new("f2".to_string(), false)),
            (F(1), false)
        );
        // Check symbol merge
        // Updating external flag is not supposed here
        assert!(obj.functions[0].is_external());
        assert_eq!(
            obj.add_function(Function::new("f1".to_string(), false)),
            (F(0), true)
        );
        assert!(obj.functions[0].is_external());
        assert!(!obj.functions[1].is_external());
    }
    
    #[test]
    fn test_link() {
        let mut obj1 = ObjectFile::new();
        let mut obj2 = ObjectFile::new();
        
        // Init object 1
        assert_eq!(
            obj1.add_function(Function::new("static11".to_string(), false)),
            (F(0), false)
        );
        assert_eq!(
            obj1.add_function(Function::new("static12".to_string(), false)),
            (F(1), false)
        );
        assert_eq!(
            obj1.add_function(Function::new("shared1".to_string(), false)),
            (F(2), false)
        );
        assert_eq!(
            obj1.add_function(Function::new("shared2".to_string(), true)),
            (F(3), false)
        );
        assert_eq!(
            obj1.add_object(
                Object::new(vec![None, Some(F(0)), Some(F(3))]),
            ),
            O(0)
        );
        assert_eq!(
            obj1.add_points_to(
                PointsTo::new(vec![F(1), F(3), O(0)])
            ),
            P(0)
        );
        assert_eq!(
            obj1.add_points_to(
                PointsTo::new(vec![F(1), F(3)])
            ),
            P(1)
        );
        assert_eq!(
            obj1.add_fcall(
                FCall::new(
                    F(0), 
                    vec![Some(F(1)), None, Some(F(3))]
                )
            ),
            C(0)
        );
        assert_eq!(
            obj1.add_fcall(
                FCall::new_with_callsite(
                    P(0),
                    vec![None, Some(O(0)), Some(F(3))],
                    F(1)
                )
            ),
            C(1)
        );

        // Init object 2
        assert_eq!(
            obj2.add_function(Function::new("static21".to_string(), false)),
            (F(0), false)
        );
        assert_eq!(
            obj2.add_function(Function::new("shared1".to_string(), true)),
            (F(1), false)
        );
        assert_eq!(
            obj2.add_function(Function::new("static22".to_string(), false)),
            (F(2), false)
        );
        assert_eq!(
            obj2.add_function(Function::new("shared2".to_string(), false)),
            (F(3), false)
        );
        assert_eq!(
            obj2.add_object(
                Object::new(vec![Some(F(1)), Some(F(1))]),
            ),
            O(0)
        );
        assert_eq!(
            obj2.add_points_to(
                PointsTo::new(vec![F(1), F(3), O(0), F(2), F(0)])
            ),
            P(0)
        );
        assert_eq!(
            obj2.add_points_to(
                PointsTo::new(vec![F(0), F(1), F(3)])
            ),
            P(1)
        );
        assert_eq!(
            obj2.add_fcall(
                FCall::new_with_callsite(
                    P(0),
                    vec![None, Some(P(1)), None],
                    F(3)
                )
            ),
            C(0)
        );
        assert_eq!(
            obj2.add_fcall(
                FCall::new(
                    F(1),
                    vec![Some(O(0)), Some(F(1))]
                )
            ),
            C(1)
        );
        // Link objects now
        obj1.link(obj2);
        
        // Check functions
        let total_f = [
            "static11", "static12", "shared1", "shared2", "static21", "static22"
        ];
        assert_eq!(total_f.len(), obj1.functions.len());
        for (i, f) in total_f.iter().enumerate() {
            assert_eq!(
                obj1.get_fun_id(f),
                Some(i)
            );
        }
        assert!(!obj1.get_fun_by_name("shared1").unwrap().is_external());
        assert!(!obj1.get_fun_by_name("shared2").unwrap().is_external());

        // Check objects
        assert_eq!(obj1.objects.len(), 2);
        assert_eq!(
            obj1.objects[0],
            Object::new(vec![None, Some(F(0)), Some(F(3))])
        );
        assert_eq!(
            obj1.objects[1],
            Object::new(vec![Some(F(2)), Some(F(2))])
        );
        
        // Check points-to
        assert_eq!(obj1.points_to.len(), 4);
        assert_eq!(
            obj1.points_to[0],
            PointsTo::new(vec![F(1), F(3), O(0)])
        );
        assert_eq!(
            obj1.points_to[1],
            PointsTo::new(vec![F(1), F(3)])
        );
        assert_eq!(
            obj1.points_to[2],
            PointsTo::new(vec![F(2), F(3), O(1), F(5), F(4)])
        );
        assert_eq!(
            obj1.points_to[3],
            PointsTo::new(vec![F(4), F(2), F(3)])
        );
        
        // Check calls
        assert_eq!(obj1.calls.len(), 4);
        assert_eq!(
            obj1.calls[0],
            FCall::new(
                F(0),
                vec![Some(F(1)), None, Some(F(3))]
            )
        );
        assert_eq!(
            obj1.calls[1],
            FCall::new_with_callsite(
                P(0),
                vec![None, Some(O(0)), Some(F(3))],
                F(1)
            )
        );
        assert_eq!(
            obj1.calls[2],
            FCall::new_with_callsite(
                P(2),
                vec![None, Some(P(3)), None],
                F(3)
            )
        );
        assert_eq!(
            obj1.calls[3],
            FCall::new(
                F(2),
                vec![Some(O(1)), Some(F(2))]
            )
        );
    }
}
