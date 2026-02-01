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
/// Items for which `must_include(i)` returns true are always kept.
#[allow(
    clippy::cognitive_complexity,
    reason = "Single function mirrors JSON streaming sampler phases"
)]
pub fn choose_indices_default(
    total: usize,
    cap: usize,
    must_include: impl Fn(usize) -> bool,
) -> Vec<usize> {
    if cap == 0 || total == 0 {
        return collect_required(total, cap, &must_include);
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
        return merge_required(out, total, cap, &must_include);
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
        return merge_required(out, total, cap, &must_include);
    }
    // Random phase: use accept_index on logical index to thin remaining
    while out.len() < cap && idx < total {
        if accept_index(idx as u64) {
            out.push(idx);
        }
        idx += 1;
    }
    merge_required(out, total, cap, &must_include)
}

/// Choose head prefix indices.
/// Items for which `must_include(i)` returns true are always kept.
pub fn choose_indices_head(
    total: usize,
    cap: usize,
    must_include: impl Fn(usize) -> bool,
) -> Vec<usize> {
    let kept = total.min(cap);
    let out: Vec<usize> = (0..kept).collect();
    merge_required(out, total, cap, &must_include)
}

/// Choose tail suffix indices.
/// Items for which `must_include(i)` returns true are always kept.
pub fn choose_indices_tail(
    total: usize,
    cap: usize,
    must_include: impl Fn(usize) -> bool,
) -> Vec<usize> {
    if cap == 0 || total == 0 {
        return collect_required(total, cap, &must_include);
    }
    let kept = total.min(cap);
    let start = total.saturating_sub(kept);
    let out: Vec<usize> = (start..total).collect();
    merge_required(out, total, cap, &must_include)
}

/// Dispatcher: choose indices for a given sampler kind.
/// Items for which `must_include(i)` returns true are always kept,
/// regardless of the sampling strategy or cap.
pub fn choose_indices(
    kind: ArraySamplerKind,
    total: usize,
    cap: usize,
    must_include: impl Fn(usize) -> bool,
) -> Vec<usize> {
    match kind {
        ArraySamplerKind::Default => {
            choose_indices_default(total, cap, must_include)
        }
        ArraySamplerKind::Head => {
            choose_indices_head(total, cap, must_include)
        }
        ArraySamplerKind::Tail => {
            choose_indices_tail(total, cap, must_include)
        }
    }
}

/// Merge required indices into an already-chosen set, preserving sorted order.
/// At most `cap` extra required indices are added (sampled from the required
/// set using the same head/mid/tail distribution) to avoid blowing up when
/// most items match.
fn merge_required(
    sampled: Vec<usize>,
    total: usize,
    cap: usize,
    must_include: &impl Fn(usize) -> bool,
) -> Vec<usize> {
    let mut seen = vec![false; total];
    for &i in &sampled {
        seen[i] = true;
    }
    let mut extra: Vec<usize> = Vec::new();
    for i in 0..total {
        if !seen[i] && must_include(i) {
            extra.push(i);
        }
    }
    if extra.is_empty() {
        return sampled;
    }
    // Sub-sample the extras so we don't blow past the cap.
    if extra.len() > cap {
        let sub = subsample_indices(extra.len(), cap);
        extra = sub.into_iter().map(|i| extra[i]).collect();
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

/// Collect only the required indices (used when cap is 0).
fn collect_required(
    total: usize,
    cap: usize,
    must_include: &impl Fn(usize) -> bool,
) -> Vec<usize> {
    let all: Vec<usize> = (0..total).filter(|&i| must_include(i)).collect();
    if all.len() <= cap || cap == 0 {
        return all;
    }
    let sub = subsample_indices(all.len(), cap);
    sub.into_iter().map(|i| all[i]).collect()
}

/// Pure default-policy sub-sampling with no `must_include` (breaks recursion).
fn subsample_indices(total: usize, cap: usize) -> Vec<usize> {
    choose_indices_default(total, cap, |_| false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sampler_returns_all_when_cap_not_binding() {
        let total = 10usize;
        let cap = total + 5;
        let indices = choose_indices_default(total, cap, |_| false);
        assert_eq!(indices, (0..total).collect::<Vec<_>>());
    }

    #[test]
    fn default_sampler_respects_cap_when_smaller() {
        let total = 10usize;
        let cap = 3usize;
        let indices = choose_indices_default(total, cap, |_| false);
        assert!(indices.len() <= cap);
    }

    #[test]
    fn must_include_adds_missing_indices() {
        let total = 20usize;
        let cap = 3usize;
        // Force index 15 to be included even though cap is 3
        let indices = choose_indices_default(total, cap, |i| i == 15);
        assert!(
            indices.contains(&15),
            "must_include index should be present: {indices:?}"
        );
        // Original sampled indices should still be present
        assert!(indices.contains(&0), "head items should be present");
    }

    #[test]
    fn must_include_preserves_sorted_order() {
        let total = 100usize;
        let cap = 5usize;
        let indices =
            choose_indices_default(total, cap, |i| i == 50 || i == 90);
        for w in indices.windows(2) {
            assert!(w[0] < w[1], "indices should be sorted: {indices:?}");
        }
        assert!(indices.contains(&50));
        assert!(indices.contains(&90));
    }

    #[test]
    fn must_include_with_zero_cap() {
        let total = 10usize;
        let indices =
            choose_indices_default(total, 0, |i| i == 3 || i == 7);
        assert_eq!(indices, vec![3, 7]);
    }

    #[test]
    fn must_include_no_duplicates_when_already_sampled() {
        let total = 10usize;
        let cap = 10usize;
        // All indices already sampled; must_include shouldn't duplicate
        let indices = choose_indices_default(total, cap, |i| i == 0);
        assert_eq!(indices, (0..total).collect::<Vec<_>>());
    }
}
