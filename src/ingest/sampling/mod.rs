use crate::ArraySamplerStrategy;

/// Ingest-agnostic array sampling strategies.
///
/// These functions return original element indices to keep. Callers are
/// expected to materialize children in the returned order and, when the
/// selection is non-contiguous, record `arr_indices` so renderers can denote
/// internal gaps.
#[derive(Copy, Clone, Debug, Default)]
pub enum ArraySamplerKind {
    #[default]
    Default,
    Head,
    Tail,
}

impl From<ArraySamplerStrategy> for ArraySamplerKind {
    fn from(strategy: ArraySamplerStrategy) -> Self {
        match strategy {
            ArraySamplerStrategy::Default => ArraySamplerKind::Default,
            ArraySamplerStrategy::Head => ArraySamplerKind::Head,
            ArraySamplerStrategy::Tail => ArraySamplerKind::Tail,
        }
    }
}

// Default policy parameters:
// - first N: ensure early coverage of the head
// - greedy: take a portion of the remaining capacity linearly
// - random: index-hash acceptance to spread the rest (~50%)
const RANDOM_ACCEPT_SEED: u64 = 0x9e37_79b9_7f4a_7c15;
const RANDOM_ACCEPT_THRESHOLD: u32 = 0x8000_0000; // ~50%
const KEEP_FIRST_COUNT: usize = 3;
const GREEDY_PORTION_DIVISOR: usize = 2;

fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn accept_index(i: u64) -> bool {
    let h = mix64(i ^ RANDOM_ACCEPT_SEED);
    ((h >> 32) as u32) < RANDOM_ACCEPT_THRESHOLD
}

/// Choose indices using the default policy (keep-first, greedy, random accept).
#[allow(
    clippy::cognitive_complexity,
    reason = "Single function mirrors JSON streaming sampler phases"
)]
pub fn choose_indices_default(total: usize, cap: usize) -> Vec<usize> {
    if cap == 0 || total == 0 {
        return Vec::new();
    }
    if cap >= total {
        return (0..total).collect();
    }
    let mut out = Vec::with_capacity(cap.min(4096));
    // Keep-first phase
    let keep_first = KEEP_FIRST_COUNT.min(cap).min(total);
    for i in 0..keep_first {
        out.push(i);
    }
    if out.len() >= cap || out.len() >= total {
        out.truncate(cap.min(total));
        return out;
    }
    // Greedy phase: take a portion of remaining capacity linearly
    let mut idx = keep_first;
    let greedy_remaining =
        (cap.saturating_sub(keep_first)) / GREEDY_PORTION_DIVISOR;
    let mut g = 0usize;
    while out.len() < cap && g < greedy_remaining && idx < total {
        out.push(idx);
        idx += 1;
        g += 1;
    }
    if out.len() >= cap || idx >= total {
        return out;
    }
    // Random phase: use accept_index on logical index to thin remaining
    while out.len() < cap && idx < total {
        if accept_index(idx as u64) {
            out.push(idx);
        }
        idx += 1;
    }
    out
}

/// Choose head prefix indices.
pub fn choose_indices_head(total: usize, cap: usize) -> Vec<usize> {
    let kept = total.min(cap);
    (0..kept).collect()
}

/// Choose tail suffix indices.
pub fn choose_indices_tail(total: usize, cap: usize) -> Vec<usize> {
    if cap == 0 || total == 0 {
        return Vec::new();
    }
    let kept = total.min(cap);
    let start = total.saturating_sub(kept);
    (start..total).collect()
}

/// Dispatcher: choose indices for a given sampler kind.
pub fn choose_indices(
    kind: ArraySamplerKind,
    total: usize,
    cap: usize,
) -> Vec<usize> {
    match kind {
        ArraySamplerKind::Default => choose_indices_default(total, cap),
        ArraySamplerKind::Head => choose_indices_head(total, cap),
        ArraySamplerKind::Tail => choose_indices_tail(total, cap),
    }
}

/// Merge required indices into an already-chosen set, preserving sorted order.
///
/// Use this as a post-step after `choose_indices` when certain indices must
/// be unconditionally kept (e.g., JSONL lines matching a grep pattern).
#[allow(
    clippy::cognitive_complexity,
    reason = "Linear collect-and-merge logic reads clearest as a single function"
)]
pub fn merge_required(
    sampled: Vec<usize>,
    total: usize,
    must_include: &impl Fn(usize) -> bool,
) -> Vec<usize> {
    let mut seen = vec![false; total];
    for &i in &sampled {
        seen[i] = true;
    }
    let mut extra: Vec<usize> = Vec::new();
    for (i, &already) in seen.iter().enumerate() {
        if !already && must_include(i) {
            extra.push(i);
        }
    }
    if extra.is_empty() {
        return sampled;
    }
    // Merge both sorted sequences
    let mut result = Vec::with_capacity(sampled.len() + extra.len());
    let (mut si, mut ei) = (0, 0);
    while si < sampled.len() && ei < extra.len() {
        if sampled[si] <= extra[ei] {
            result.push(sampled[si]);
            si += 1;
        } else {
            result.push(extra[ei]);
            ei += 1;
        }
    }
    result.extend_from_slice(&sampled[si..]);
    result.extend_from_slice(&extra[ei..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sampler_returns_all_when_cap_not_binding() {
        let total = 10usize;
        let cap = total + 5;
        let indices = choose_indices_default(total, cap);
        assert_eq!(indices, (0..total).collect::<Vec<_>>());
    }

    #[test]
    fn default_sampler_respects_cap_when_smaller() {
        let total = 10usize;
        let cap = 3usize;
        let indices = choose_indices_default(total, cap);
        assert!(indices.len() <= cap);
    }

    #[test]
    fn merge_required_adds_missing_indices() {
        let total = 20usize;
        let cap = 3usize;
        let sampled = choose_indices_default(total, cap);
        // Force index 15 to be included even though cap is 3
        let indices = merge_required(sampled, total, &|i| i == 15);
        assert!(
            indices.contains(&15),
            "must_include index should be present: {indices:?}"
        );
        // Original sampled indices should still be present
        assert!(indices.contains(&0), "head items should be present");
    }

    #[test]
    fn merge_required_preserves_sorted_order() {
        let total = 100usize;
        let cap = 5usize;
        let sampled = choose_indices_default(total, cap);
        let indices = merge_required(sampled, total, &|i| i == 50 || i == 90);
        for w in indices.windows(2) {
            assert!(w[0] < w[1], "indices should be sorted: {indices:?}");
        }
        assert!(indices.contains(&50));
        assert!(indices.contains(&90));
    }

    #[test]
    fn merge_required_with_zero_cap() {
        let total = 10usize;
        let sampled = choose_indices_default(total, 0);
        let indices = merge_required(sampled, total, &|i| i == 3 || i == 7);
        assert_eq!(indices, vec![3, 7]);
    }

    #[test]
    fn merge_required_no_duplicates_when_already_sampled() {
        let total = 10usize;
        let cap = 10usize;
        let sampled = choose_indices_default(total, cap);
        // All indices already sampled; must_include shouldn't duplicate
        let indices = merge_required(sampled, total, &|i| i == 0);
        assert_eq!(indices, (0..total).collect::<Vec<_>>());
    }

    #[test]
    fn head_sampler_merge_includes_required_beyond_cap() {
        let total = 20usize;
        let cap = 3usize;
        let sampled = choose_indices_head(total, cap);
        // Head keeps 0,1,2 — force index 17 to also be included
        let indices = merge_required(sampled, total, &|i| i == 17);
        assert_eq!(&indices[..3], &[0, 1, 2]);
        assert!(
            indices.contains(&17),
            "must_include index should be present: {indices:?}"
        );
        for w in indices.windows(2) {
            assert!(w[0] < w[1], "indices should be sorted: {indices:?}");
        }
    }

    #[test]
    fn head_sampler_merge_no_duplicates_when_already_sampled() {
        let total = 10usize;
        let cap = 5usize;
        let sampled = choose_indices_head(total, cap);
        // Index 2 is already in head range 0..5
        let indices = merge_required(sampled, total, &|i| i == 2);
        assert_eq!(indices, (0..5).collect::<Vec<_>>());
    }

    #[test]
    fn tail_sampler_merge_includes_required_beyond_cap() {
        let total = 20usize;
        let cap = 3usize;
        let sampled = choose_indices_tail(total, cap);
        // Tail keeps 17,18,19 — force index 2 to also be included
        let indices = merge_required(sampled, total, &|i| i == 2);
        assert!(indices.contains(&2), "must_include index should be present");
        assert!(indices.contains(&17));
        assert_eq!(indices, vec![2, 17, 18, 19]);
    }

    #[test]
    fn tail_sampler_merge_no_duplicates_when_already_sampled() {
        let total = 10usize;
        let cap = 5usize;
        let sampled = choose_indices_tail(total, cap);
        // Index 7 is already in tail range 5..10
        let indices = merge_required(sampled, total, &|i| i == 7);
        assert_eq!(indices, (5..10).collect::<Vec<_>>());
    }

    #[test]
    fn tail_sampler_merge_with_zero_cap_returns_only_required() {
        let total = 10usize;
        let sampled = choose_indices_tail(total, 0);
        let indices = merge_required(sampled, total, &|i| i == 4 || i == 8);
        assert_eq!(indices, vec![4, 8]);
    }

    #[test]
    fn head_sampler_merge_with_zero_cap_returns_only_required() {
        let total = 10usize;
        let sampled = choose_indices_head(total, 0);
        let indices = merge_required(sampled, total, &|i| i == 4 || i == 8);
        assert_eq!(indices, vec![4, 8]);
    }
}
