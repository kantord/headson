use crate::order::{NodeId, PriorityOrder};

/// Seed the work stack by including the first `k` nodes in global priority order.
fn seed_stack_with_top_k(
    order: &PriorityOrder,
    k: usize,
    inclusion_flags: &mut [u32],
    render_id: u32,
    work_stack: &mut Vec<NodeId>,
) {
    for &id in order.by_priority.iter().take(k) {
        let idx = id.0;
        if inclusion_flags[idx] != render_id {
            inclusion_flags[idx] = render_id;
            work_stack.push(id);
        }
    }
}

/// Pop from the work stack; for each node include its parent; continue until empty.
fn propagate_marks_to_ancestors(
    parent: &[Option<NodeId>],
    inclusion_flags: &mut [u32],
    render_id: u32,
    work_stack: &mut Vec<NodeId>,
) {
    while let Some(id) = work_stack.pop() {
        let idx = id.0;
        match parent[idx] {
            Some(parent) if inclusion_flags[parent.0] != render_id => {
                inclusion_flags[parent.0] = render_id;
                work_stack.push(parent);
            }
            _ => {}
        }
    }
}

/// Include the first `k` nodes by global priority order and all of their ancestors
/// in the current render set (identified by `render_id`).
pub(crate) fn mark_top_k_and_ancestors(
    order: &PriorityOrder,
    k: usize,
    inclusion_flags: &mut [u32],
    render_id: u32,
) {
    let mut work_stack: Vec<NodeId> = Vec::new();
    seed_stack_with_top_k(
        order,
        k,
        inclusion_flags,
        render_id,
        &mut work_stack,
    );
    propagate_marks_to_ancestors(
        &order.parent,
        inclusion_flags,
        render_id,
        &mut work_stack,
    );
}
