use crate::order::{NodeId, PriorityOrder};

/// Seed the work stack by marking the first `k` nodes in global order.
fn seed_stack_with_top_k(
    order: &PriorityOrder,
    k: usize,
    marks: &mut [u32],
    mark_gen: u32,
    work_stack: &mut Vec<NodeId>,
) {
    for &id in order.order.iter().take(k) {
        let idx = id.0;
        if marks[idx] != mark_gen {
            marks[idx] = mark_gen;
            work_stack.push(id);
        }
    }
}

/// Pop from the work stack; for each node mark its parent; continue until empty.
fn propagate_marks_to_ancestors(
    parent: &[Option<NodeId>],
    marks: &mut [u32],
    mark_gen: u32,
    work_stack: &mut Vec<NodeId>,
) {
    while let Some(id) = work_stack.pop() {
        let idx = id.0;
        match parent[idx] {
            Some(parent) if marks[parent.0] != mark_gen => {
                marks[parent.0] = mark_gen;
                work_stack.push(parent);
            }
            _ => {}
        }
    }
}

/// Mark the first `k` nodes by global order and all of their ancestors.
pub(crate) fn mark_top_k_and_ancestors(
    order: &PriorityOrder,
    k: usize,
    marks: &mut [u32],
    mark_gen: u32,
) {
    let mut work_stack: Vec<NodeId> = Vec::new();
    seed_stack_with_top_k(order, k, marks, mark_gen, &mut work_stack);
    propagate_marks_to_ancestors(
        &order.parent,
        marks,
        mark_gen,
        &mut work_stack,
    );
}
