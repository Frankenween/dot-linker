use std::hash::Hash;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct Function {
    name: String,
    is_external: bool,
}

impl Function {
    pub fn new(name: String, is_external: bool) -> Self {
        Self { name, is_external }
    }
    
    pub fn get_name(&self) -> &String {
        &self.name
    }
    
    pub fn is_external(&self) -> bool { 
       self.is_external
    }
    
    pub fn set_external(&mut self, is_external: bool) {
        self.is_external = is_external;
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
/// Representation of points-to set
/// Contract: points-to sets are flattened, so it may point to Function or Object only
pub struct PointsTo<SymPtr> {
    pub points_to: Vec<SymPtr>,
}

impl<SymPtr> PointsTo<SymPtr> {
    pub fn new(points_to: Vec<SymPtr>) -> Self {
        Self { points_to }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
/// Data structure representation
pub struct Object<SymPtr> {
    /// Struct fields. Non-pointer fields are None
    pub fields: Vec<Option<SymPtr>>,
}

impl<SymPtr> Object<SymPtr> {
    pub fn new(fields: Vec<Option<SymPtr>>) -> Self {
        Self { fields }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct FCall<SymPtr> {
    pub callee: SymPtr,
    /// Function arguments. None for non-pointer args
    pub arguments: Vec<Option<SymPtr>>
}

impl<SymPtr> FCall<SymPtr> {
    pub fn new(callee: SymPtr, arguments: Vec<Option<SymPtr>>) -> Self {
        Self { callee, arguments }
    }
}
