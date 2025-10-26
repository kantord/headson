use serde::de::{IgnoredAny, SeqAccess};

use super::{JsonTreeBuilder, SampledArray};

pub(crate) fn sample_stream<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
) -> Result<SampledArray, A::Error>
where
    A: SeqAccess<'de>,
{
    if cap == 0 {
        let total = drain_len(seq)?;
        return Ok(SampledArray {
            children: Vec::new(),
            indices: Vec::new(),
            total_len: total,
        });
    }
    let k = cap;
    let mut ring_idx: Vec<usize> = vec![0; k];
    let mut ring_children: Vec<usize> = vec![0; k];
    let mut count = 0usize;
    let mut head = 0usize;
    loop {
        let seed = builder.seed();
        match seq.next_element_seed(seed)? {
            Some(child_id) => {
                ring_idx[head] = count;
                ring_children[head] = child_id;
                head = if head + 1 == k { 0 } else { head + 1 };
                count = count.saturating_add(1);
            }
            None => break,
        }
    }
    Ok(materialize_tail(&ring_idx, &ring_children, count, head, k))
}

fn drain_len<'de, A>(seq: &mut A) -> Result<usize, A::Error>
where
    A: SeqAccess<'de>,
{
    let mut total = 0usize;
    while (seq.next_element::<IgnoredAny>()?).is_some() {
        total += 1;
    }
    Ok(total)
}

fn materialize_tail(
    ring_idx: &[usize],
    ring_children: &[usize],
    count: usize,
    head: usize,
    k: usize,
) -> SampledArray {
    let kept = count.min(k);
    if kept == 0 {
        return SampledArray {
            children: Vec::new(),
            indices: Vec::new(),
            total_len: count,
        };
    }
    let start = if count >= k { head } else { 0 };
    let mut children = Vec::with_capacity(kept);
    let mut indices = Vec::with_capacity(kept);
    for i in 0..kept {
        let pos = (start + i) % k;
        indices.push(ring_idx[pos]);
        children.push(ring_children[pos]);
    }
    SampledArray {
        children,
        indices,
        total_len: count,
    }
}

#[cfg(test)]
mod tests {
    use crate::order::PriorityConfig;

    #[test]
    fn tail_sampler_keeps_last_n_indices() {
        let input = b"[0,1,2,3,4,5,6,7,8,9]".to_vec();
        let mut cfg = PriorityConfig::new(usize::MAX, 5);
        cfg.array_sampler = crate::ArraySamplerStrategy::Tail;
        let arena =
            crate::json_ingest::build_json_tree_arena_from_bytes(input, &cfg)
                .expect("arena");
        let root = &arena.nodes[arena.root_id];
        assert_eq!(root.children_len, 5, "kept 5");
        let mut orig_indices = Vec::new();
        for i in 0..root.children_len {
            let oi = if root.arr_indices_len > 0 {
                arena.arr_indices[root.arr_indices_start + i]
            } else {
                i
            };
            orig_indices.push(oi);
        }
        assert_eq!(orig_indices, vec![5, 6, 7, 8, 9]);
    }
}
