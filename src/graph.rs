use std::collections::{HashMap, HashSet};

pub type NodeId = usize;

#[derive(Clone)]
pub struct TypedGraph<T> {
    next: Vec<Vec<NodeId>>,
    mapping: Vec<T>
}

impl <T> Default for TypedGraph<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl <T> TypedGraph<T> {
    pub fn new() -> Self {
        Self {
            next: vec![],
            mapping: vec![],
        }
    }

    pub fn new_with_size(size: usize) -> Self
    where T: Default {
        Self {
            next: vec![vec![]; size],
            mapping: (0..size).map(|_| Default::default()).collect(),
        }
    }

    pub fn new_with_mapping(mapping: Vec<T>) -> Self {
        Self {
            next: vec![vec![]; mapping.len()],
            mapping,
        }
    }

    pub fn size(&self) -> usize {
        self.next.len()
    }

    pub fn new_node(&mut self, value: T) -> NodeId {
        let id = self.next.len();
        self.next.push(vec![]);
        self.mapping.push(value);
        id
    }

    pub fn add_edge(&mut self, from: NodeId, to: NodeId) {
        self.next[from].push(to);
    }

    pub fn next(&self, id: NodeId) -> &Vec<NodeId> {
        &self.next[id]
    }

    pub fn mapping(&self) -> &Vec<T> {
        &self.mapping
    }

    pub fn mapping_mut(&mut self) -> &mut Vec<T> {
        &mut self.mapping
    }

    pub fn map<F, R>(self, f: F) -> TypedGraph<R>
    where F: FnMut(T) -> R {
        TypedGraph {
            next: self.next,
            mapping: self.mapping.into_iter().map(f).collect()
        }
    }

    fn mark_reachable(&self, start: NodeId, used: &mut [bool]) {
        let mut stack = vec![start];
        used[start] = true;
        while let Some(v) = stack.pop() {
            for &u in &self.next[v] {
                if !used[u] {
                    stack.push(u);
                    used[u] = true;
                }
            }
        }
    }

    /// Get list of all nodes, reachable from any start node
    pub fn get_reachable(&self, nodes: &[NodeId]) -> Vec<NodeId> {
        let mut used = vec![false; self.size()];
        for node in nodes {
            self.mark_reachable(*node, &mut used);
        }
        used.iter()
            .enumerate()
            .filter_map(|(i, &v)| if v { Some(i) } else { None })
            .collect()
    }

    /// Construct inverse graph
    /// Node numbers are preserved
    pub fn inv(&self) -> TypedGraph<T>
    where T: Clone {
        let mut graph = TypedGraph::new_with_mapping(self.mapping().clone());
        for v in 0..self.size() {
            for &u in &self.next[v] {
                graph.add_edge(u, v);
            }
        }
        graph
    }

    /// Create new graph with specified nodes only.
    /// Mapping NewNode -> OriginalNode is also provided
    pub fn projection(&self, nodes: &[NodeId]) -> (TypedGraph<T>, Vec<NodeId>)
    where T: Clone {
        let distinct: HashSet<NodeId> = HashSet::from_iter(nodes.iter().cloned());

        let mut mapping = distinct.iter().cloned().collect::<Vec<NodeId>>();
        mapping.sort();
        let mut graph = TypedGraph::new_with_mapping(
            mapping
                .clone()
                .iter()
                .map(|&v| self.mapping[v].clone())
                .collect()
        );


        let mut inv_mapping: HashMap<NodeId, NodeId> = HashMap::new();
        for (v, &orig) in mapping.iter().enumerate() {
            inv_mapping.insert(orig, v);
        }

        for (v, &orig) in mapping.iter().enumerate() {
            for u_orig in &self.next[orig] {
                if let Some(&u) = inv_mapping.get(u_orig) {
                    graph.add_edge(v, u);
                }
            }
        }
        (graph, mapping)
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::TypedGraph;

    fn get_sample_graph() -> TypedGraph<u8> {
        let mut graph = TypedGraph::new_with_size(10);
        graph.add_edge(0, 1);
        graph.add_edge(1, 3);
        graph.add_edge(2, 0);
        graph.add_edge(3, 4);
        graph.add_edge(3, 6);
        graph.add_edge(4, 1);
        graph.add_edge(5, 4);
        graph.add_edge(6, 6);
        graph.add_edge(7, 8);
        graph.add_edge(9, 8);
        graph
    }

    #[test]
    fn test_reachable() {
        let graph = get_sample_graph();
        assert_eq!(graph.get_reachable(&[0]), vec![0, 1, 3, 4, 6]);
        assert_eq!(graph.get_reachable(&[6]), vec![6]);
        assert_eq!(graph.get_reachable(&[5, 9]), vec![1, 3, 4, 5, 6, 8, 9])
    }
}
