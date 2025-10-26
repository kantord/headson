//! Deterministic, low-overhead array sampling for streaming ingest.
//!
//! Goals:
//! - Single pass over the input (serde streaming), no backtracking.
//! - Parse only selected elements; skip others via `IgnoredAny`.
//! - Stable, deterministic selection via a cheap 64-bit mix (SplitMix64-like)
//!   with a fixed, cap-independent seed.
//! - Preserve edges and coverage: always include a small head prefix (first 3),
//!   then greedily include part of the head of the middle, then include more
//!   items by a fixed predicate until the per-array cap is reached.
//! - Record original indices and total length for accurate omission info and
//!   internal gap markers in display templates.
//!
//! Rationale:
//! - Prefix-only ingest is very fast but never sees mid/tail.
//! - Reservoir sampling often implies extra parsing/replacements or RNG state.
//! - A stateless, per-index inclusion test keeps costs tiny and predictable.
//! - Hashing indices avoids periodic aliasing in selection.

use serde::de::{IgnoredAny, SeqAccess};

use super::builder::JsonTreeBuilder;

/// Result of sampling a streamed array.
/// - `children`: arena ids of kept children in kept order
/// - `indices`: original indices of the kept children (may be empty if contiguous)
/// - `total_len`: total number of elements encountered in the array
pub(crate) struct SampledArray {
    pub children: Vec<usize>,
    pub indices: Vec<usize>,
    pub total_len: usize,
}

struct PhaseState {
    idx: usize,
    kept: usize,
}

struct SampleOut<'a> {
    children: &'a mut Vec<usize>,
    indices: &'a mut Vec<usize>,
}

#[inline]
fn mix64(mut x: u64) -> u64 {
    // SplitMix64-style mixer: cheap and with good avalanche
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

// Deterministic, cap-independent acceptance predicate.
// Uses a fixed seed so the accepted set per index is stable across budgets.
#[inline]
fn accept_index(i: u64) -> bool {
    const SEED: u64 = 0x9e37_79b9_7f4a_7c15;
    const THRESH: u32 = 0x8000_0000; // ~50%
    let h = mix64(i ^ SEED);
    ((h >> 32) as u32) < THRESH
}

/// Sample an array from a serde `SeqAccess` without backtracking.
///
/// Returns (children_ids, original_indices, total_length).
#[inline]
fn parse_keep<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    idx: usize,
    out: &mut SampleOut<'_>,
) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    let de = builder.seed();
    match seq.next_element_seed(de)? {
        Some(c) => {
            out.children.push(c);
            out.indices.push(idx);
            Ok(true)
        }
        None => Ok(false),
    }
}

#[inline]
fn skip_one<'de, A>(seq: &mut A) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    Ok(seq.next_element::<IgnoredAny>()?.is_some())
}

#[inline]
fn phase_keep_first<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
    keep_first: usize,
    state: &mut PhaseState,
    out: &mut SampleOut<'_>,
) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    while state.kept < cap && state.idx < keep_first {
        if !parse_keep(seq, builder, state.idx, out)? {
            return Ok(true);
        }
        state.kept += 1;
        state.idx = state.idx.saturating_add(1);
    }
    Ok(false)
}

#[inline]
fn phase_greedy<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
    greedy_remaining: &mut usize,
    state: &mut PhaseState,
    out: &mut SampleOut<'_>,
) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    while state.kept < cap && *greedy_remaining > 0 {
        if !parse_keep(seq, builder, state.idx, out)? {
            return Ok(true);
        }
        state.kept += 1;
        *greedy_remaining = greedy_remaining.saturating_sub(1);
        state.idx = state.idx.saturating_add(1);
    }
    Ok(false)
}

#[inline]
fn phase_random<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
    state: &mut PhaseState,
    out: &mut SampleOut<'_>,
) -> Result<(), A::Error>
where
    A: SeqAccess<'de>,
{
    while state.kept < cap {
        if accept_index(state.idx as u64) {
            if !parse_keep(seq, builder, state.idx, out)? {
                return Ok(());
            }
            state.kept += 1;
        } else if !skip_one(seq)? {
            break;
        }
        state.idx = state.idx.saturating_add(1);
    }
    Ok(())
}

pub(crate) fn sample_stream<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
) -> Result<SampledArray, A::Error>
where
    A: SeqAccess<'de>,
{
    if cap == 0 {
        // Drain the sequence cheaply and report total length
        let mut total = 0usize;
        while (seq.next_element::<IgnoredAny>()?).is_some() {
            total += 1;
        }
        return Ok(SampledArray {
            children: Vec::new(),
            indices: Vec::new(),
            total_len: total,
        });
    }

    let mut local_children: Vec<usize> = Vec::new();
    let mut local_indices: Vec<usize> = Vec::new();
    // Reserve conservatively to avoid huge allocations when cap is large
    let reserve = cap.min(4096);
    local_children.reserve(reserve);
    local_indices.reserve(reserve);

    let mut state = PhaseState { idx: 0, kept: 0 };

    // Always keep the first F items (up to cap)
    const F: usize = 3;
    let keep_first = F.min(cap);
    // Greedy head-of-middle: take about half of the remaining budget eagerly
    let mut greedy_remaining = (cap.saturating_sub(keep_first)) / 2;

    // Phase 1: keep first few
    if phase_keep_first(
        seq,
        builder,
        cap,
        keep_first,
        &mut state,
        &mut SampleOut {
            children: &mut local_children,
            indices: &mut local_indices,
        },
    )? {
        return Ok(SampledArray {
            children: local_children,
            indices: local_indices,
            total_len: state.idx,
        });
    }
    // Phase 2: greedy middle head
    if phase_greedy(
        seq,
        builder,
        cap,
        &mut greedy_remaining,
        &mut state,
        &mut SampleOut {
            children: &mut local_children,
            indices: &mut local_indices,
        },
    )? {
        return Ok(SampledArray {
            children: local_children,
            indices: local_indices,
            total_len: state.idx,
        });
    }
    // Phase 3: probabilistic accept for the remainder
    phase_random(
        seq,
        builder,
        cap,
        &mut state,
        &mut SampleOut {
            children: &mut local_children,
            indices: &mut local_indices,
        },
    )?;

    // Drain the remainder to compute total length accurately
    while skip_one(seq)? {
        state.idx = state.idx.saturating_add(1);
    }

    Ok(SampledArray {
        children: local_children,
        indices: local_indices,
        total_len: state.idx,
    })
}

//
// Pluggable sampler API (enum-based, object-safe for easy storage)
//

#[derive(Copy, Clone, Debug, Default)]
pub(crate) enum ArraySamplerKind {
    #[default]
    Default,
    Tail,
}

impl ArraySamplerKind {
    pub(crate) fn sample_stream<'de, A>(
        self,
        seq: &mut A,
        builder: &JsonTreeBuilder,
        cap: usize,
    ) -> Result<SampledArray, A::Error>
    where
        A: SeqAccess<'de>,
    {
        match self {
            ArraySamplerKind::Default => {
                super::array_sample::sample_stream(seq, builder, cap)
            }
            ArraySamplerKind::Tail => sample_stream_tail(seq, builder, cap),
        }
    }
}

#[inline]
fn sample_stream_tail<'de, A>(
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
    let mut head = 0usize; // next write position modulo k
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

#[inline]
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

#[inline]
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
        // Extract original indices for children
        let mut orig_indices = Vec::new();
        for i in 0..root.children_len {
            let oi = if root.arr_indices_len > 0 {
                arena.arr_indices[root.arr_indices_start + i]
            } else {
                i
            };
            orig_indices.push(oi);
        }
        assert_eq!(
            orig_indices,
            vec![5, 6, 7, 8, 9],
            "expected last 5 indices"
        );
    }
}
