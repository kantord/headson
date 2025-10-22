use crate::order::{NodeId, PriorityOrder};

/// Mark the first `k` nodes by global order and push them onto `stack`.
pub(crate) fn mark_top_k_by_order(
    order: &PriorityOrder,
    k: usize,
    marks: &mut [u32],
    mark_gen: u32,
    stack: &mut Vec<NodeId>,
) {
    for &id in order.ids_by_order.iter().take(k) {
        let idx = id.0;
        if marks[idx] != mark_gen {
            marks[idx] = mark_gen;
            stack.push(id);
        }
    }
}

/// Pop from `stack`, and for each node mark its parent; continue until empty.
pub(crate) fn mark_ancestors_from_stack(
    parent_of: &[Option<NodeId>],
    marks: &mut [u32],
    mark_gen: u32,
    stack: &mut Vec<NodeId>,
) {
    while let Some(id) = stack.pop() {
        let idx = id.0;
        if let Some(parent) = parent_of[idx] {
            let pidx = parent.0;
            if marks[pidx] != mark_gen {
                marks[pidx] = mark_gen;
                stack.push(parent);
            }
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
    let mut stack: Vec<NodeId> = Vec::new();
    mark_top_k_by_order(order, k, marks, mark_gen, &mut stack);
    mark_ancestors_from_stack(&order.parent_of, marks, mark_gen, &mut stack);
}
