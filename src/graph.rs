use std::collections::{HashMap, HashSet};

pub type NodeId = usize;

pub struct Graph {
    next: Vec<Vec<NodeId>>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    pub fn new() -> Self {
        Self { next: vec![] }
    }

    pub fn new_with_size(size: usize) -> Self {
        Self {
            next: vec![vec![]; size],
        }
    }

    pub fn size(&self) -> usize {
        self.next.len()
    }

    pub fn new_node(&mut self) -> NodeId {
        let id = self.next.len();
        self.next.push(vec![]);
        id
    }

    pub fn add_edge(&mut self, from: NodeId, to: NodeId) {
        self.next[from].push(to);
    }

    pub fn next(&self, id: NodeId) -> &Vec<NodeId> {
        &self.next[id]
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
    pub fn inv(&self) -> Graph {
        let mut graph = Graph::new_with_size(self.size());
        for v in 0..self.size() {
            for &u in &self.next[v] {
                graph.add_edge(u, v);
            }
        }
        graph
    }

    /// Create new graph with specified nodes only.
    /// Mapping NewNode -> OriginalNode is also provided
    pub fn projection(&self, nodes: &[NodeId]) -> (Graph, Vec<NodeId>) {
        let distinct: HashSet<NodeId> = HashSet::from_iter(nodes.iter().cloned());
        let mut graph = Graph::new_with_size(distinct.len());

        let mut mapping = distinct.iter().cloned().collect::<Vec<NodeId>>();
        mapping.sort();

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
    use crate::graph::Graph;

    fn get_sample_graph() -> Graph {
        let mut graph = Graph::new_with_size(10);
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
