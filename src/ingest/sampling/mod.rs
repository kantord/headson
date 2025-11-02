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
