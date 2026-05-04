use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::error::YojanaError;

pub fn would_cycle(
    existing_edges: &[(Uuid, Uuid)],
    from: Uuid,
    to: Uuid,
) -> Result<(), YojanaError> {
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for &(src, tgt) in existing_edges {
        adj.entry(src).or_default().push(tgt);
    }
    adj.entry(from).or_default().push(to);

    let mut visited = HashSet::new();
    let mut stack = HashSet::new();

    for &node in adj.keys() {
        if !visited.contains(&node) && dfs(node, &adj, &mut visited, &mut stack) {
            return Err(YojanaError::InvalidInput(
                "adding this depends_on edge would create a cycle".into(),
            ));
        }
    }
    Ok(())
}

fn dfs(
    node: Uuid,
    adj: &HashMap<Uuid, Vec<Uuid>>,
    visited: &mut HashSet<Uuid>,
    stack: &mut HashSet<Uuid>,
) -> bool {
    visited.insert(node);
    stack.insert(node);

    if let Some(neighbors) = adj.get(&node) {
        for &next in neighbors {
            if !visited.contains(&next) {
                if dfs(next, adj, visited, stack) {
                    return true;
                }
            } else if stack.contains(&next) {
                return true;
            }
        }
    }

    stack.remove(&node);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> Uuid {
        Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, n])
    }

    #[test]
    fn no_cycle_simple_chain() {
        let edges = vec![(id(1), id(2)), (id(2), id(3))];
        assert!(would_cycle(&edges, id(3), id(4)).is_ok());
    }

    #[test]
    fn direct_self_loop() {
        assert!(would_cycle(&[], id(1), id(1)).is_err());
    }

    #[test]
    fn direct_cycle_two_nodes() {
        let edges = vec![(id(1), id(2))];
        assert!(would_cycle(&edges, id(2), id(1)).is_err());
    }

    #[test]
    fn multi_hop_cycle() {
        let edges = vec![(id(1), id(2)), (id(2), id(3)), (id(3), id(4))];
        assert!(would_cycle(&edges, id(4), id(1)).is_err());
    }

    #[test]
    fn no_cycle_diamond() {
        let edges = vec![(id(1), id(2)), (id(1), id(3)), (id(2), id(4)), (id(3), id(4))];
        assert!(would_cycle(&edges, id(4), id(5)).is_ok());
    }

    #[test]
    fn parallel_paths_no_cycle() {
        let edges = vec![(id(1), id(2)), (id(1), id(3))];
        assert!(would_cycle(&edges, id(2), id(4)).is_ok());
    }

    #[test]
    fn empty_graph_no_cycle() {
        assert!(would_cycle(&[], id(1), id(2)).is_ok());
    }
}
