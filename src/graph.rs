use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::db::EdgeRow;
use crate::error::YojanaError;
use crate::state::TaskStatus;

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

pub fn is_ready(task_id: Uuid, deps_with_status: &[(Uuid, Uuid, TaskStatus)]) -> bool {
    let my_deps: Vec<_> = deps_with_status
        .iter()
        .filter(|(src, _, _)| *src == task_id)
        .collect();
    my_deps
        .iter()
        .all(|(_, _, status)| *status == TaskStatus::Done)
}

pub fn blocked_by(task_id: Uuid, deps_with_status: &[(Uuid, Uuid, TaskStatus)]) -> Vec<Uuid> {
    deps_with_status
        .iter()
        .filter(|(src, _, status)| *src == task_id && *status != TaskStatus::Done)
        .map(|(_, tgt, _)| *tgt)
        .collect()
}

pub struct ForestNode {
    pub task_id: Uuid,
    pub children: Vec<ForestNode>,
    pub seen_before: bool,
}

/// Build a dependency forest from tasks and depends_on edges.
///
/// Direction: edge source=A target=B means "A depends on B", so B must finish
/// first. In the tree B is parent, A is child (execution order top→down).
///
/// Returns (forest_roots, standalone_task_ids).
pub fn build_dependency_forest(
    task_ids: &[Uuid],
    edges: &[EdgeRow],
) -> (Vec<ForestNode>, Vec<Uuid>) {
    let task_set: HashSet<Uuid> = task_ids.iter().copied().collect();

    // "unlocks" adjacency: target → [sources] (parent → children)
    let mut unlocks: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut has_dependency: HashSet<Uuid> = HashSet::new();
    let mut participates: HashSet<Uuid> = HashSet::new();

    for e in edges {
        if !task_set.contains(&e.source_task_id) || !task_set.contains(&e.target_task_id) {
            continue;
        }
        unlocks
            .entry(e.target_task_id)
            .or_default()
            .push(e.source_task_id);
        has_dependency.insert(e.source_task_id);
        participates.insert(e.source_task_id);
        participates.insert(e.target_task_id);
    }

    // Roots: tasks in the graph that have no dependencies themselves
    let mut roots: Vec<Uuid> = task_ids
        .iter()
        .filter(|id| participates.contains(id) && !has_dependency.contains(id))
        .copied()
        .collect();
    roots.sort();

    let standalone: Vec<Uuid> = task_ids
        .iter()
        .filter(|id| !participates.contains(id))
        .copied()
        .collect();

    let mut visited = HashSet::new();
    let forest = roots
        .into_iter()
        .map(|root| build_subtree(root, &unlocks, &mut visited))
        .collect();

    (forest, standalone)
}

fn build_subtree(
    node: Uuid,
    unlocks: &HashMap<Uuid, Vec<Uuid>>,
    visited: &mut HashSet<Uuid>,
) -> ForestNode {
    if visited.contains(&node) {
        return ForestNode {
            task_id: node,
            children: Vec::new(),
            seen_before: true,
        };
    }
    visited.insert(node);
    let children = match unlocks.get(&node) {
        Some(kids) => {
            let mut sorted = kids.clone();
            sorted.sort();
            sorted
                .into_iter()
                .map(|kid| build_subtree(kid, unlocks, visited))
                .collect()
        }
        None => Vec::new(),
    };
    ForestNode {
        task_id: node,
        children,
        seen_before: false,
    }
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
        let edges = vec![
            (id(1), id(2)),
            (id(1), id(3)),
            (id(2), id(4)),
            (id(3), id(4)),
        ];
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

    #[test]
    fn ready_no_deps() {
        let deps: Vec<(Uuid, Uuid, TaskStatus)> = vec![];
        assert!(is_ready(id(1), &deps));
    }

    #[test]
    fn ready_all_deps_done() {
        let deps = vec![
            (id(1), id(2), TaskStatus::Done),
            (id(1), id(3), TaskStatus::Done),
        ];
        assert!(is_ready(id(1), &deps));
    }

    #[test]
    fn not_ready_some_deps_pending() {
        let deps = vec![
            (id(1), id(2), TaskStatus::Done),
            (id(1), id(3), TaskStatus::InProgress),
        ];
        assert!(!is_ready(id(1), &deps));
    }

    #[test]
    fn blocked_by_returns_incomplete() {
        let deps = vec![
            (id(1), id(2), TaskStatus::Done),
            (id(1), id(3), TaskStatus::InProgress),
            (id(1), id(4), TaskStatus::NeedsTriage),
        ];
        let blockers = blocked_by(id(1), &deps);
        assert_eq!(blockers.len(), 2);
        assert!(blockers.contains(&id(3)));
        assert!(blockers.contains(&id(4)));
    }

    #[test]
    fn ready_ignores_other_tasks_deps() {
        let deps = vec![(id(2), id(3), TaskStatus::InProgress)];
        assert!(is_ready(id(1), &deps));
    }
}
