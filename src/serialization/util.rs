use crate::order::{PriorityOrder, RankedNode};

pub(crate) fn digits(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut d = 0;
    while n > 0 {
        d += 1;
        n /= 10;
    }
    d
}

fn max_index_for_child(
    order: &PriorityOrder,
    flags: &[u32],
    rid: u32,
    child: usize,
) -> Option<usize> {
    if flags.get(child).copied().unwrap_or_default() != rid {
        return None;
    }
    match order.nodes[child] {
        RankedNode::AtomicLeaf { .. } | RankedNode::SplittableLeaf { .. } => {
            order.index_in_parent_array.get(child).and_then(|idx| *idx)
        }
        RankedNode::Array { .. } | RankedNode::Object { .. } => {
            Some(compute_max_index(order, flags, rid, child))
        }
        _ => None,
    }
}

pub(crate) fn compute_max_index(
    order: &PriorityOrder,
    flags: &[u32],
    rid: u32,
    id: usize,
) -> usize {
    let mut max_idx = 0usize;
    if let Some(kids) = order.children.get(id) {
        for &cid in kids.iter() {
            let child_id = cid.0;
            if let Some(child_max) =
                max_index_for_child(order, flags, rid, child_id)
            {
                if child_max > max_idx {
                    max_idx = child_max;
                }
            }
        }
    }
    max_idx
}
