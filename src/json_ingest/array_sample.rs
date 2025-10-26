//! Deterministic, low-overhead array sampling for streaming ingest.
//!
//! Goals:
//! - Single pass over the input (serde streaming), no backtracking.
//! - Parse only selected elements; skip others via `IgnoredAny`.
//! - Deterministic selection using a cheap 64-bit mix (SplitMix64-like).
//! - Always include the first element; pick additional items up to `cap - 1`.
//! - Stable results for the same inputs (seed derived from `cap`).
//!
//! Rationale:
//! - Prefix-only ingest is very fast but never sees tail elements.
//! - Reservoir sampling often implies extra parsing/replacements or RNG state.
//! - A stateless, per-index inclusion test keeps costs tiny and predictable.
//! - We avoid periodic aliasing by hashing the index before the modulus test.

use serde::de::{IgnoredAny, SeqAccess};

use super::builder::JsonTreeBuilder;

#[inline(always)]
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
#[inline(always)]
fn accept_index(i: u64) -> bool {
    const SEED: u64 = 0x9e37_79b9_7f4a_7c15;
    const THRESH: u32 = 0x8000_0000; // ~50%
    let h = mix64(i ^ SEED);
    ((h >> 32) as u32) < THRESH
}

/// Sample an array from a serde `SeqAccess` without backtracking.
///
/// Returns (children_ids, original_indices, total_length).
#[allow(
    clippy::type_complexity,
    clippy::cognitive_complexity,
    reason = "streaming selection needs tuple return and a few branches"
)]
pub(crate) fn sample_stream<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
) -> Result<(Vec<usize>, Vec<usize>, usize), A::Error>
where
    A: SeqAccess<'de>,
{
    if cap == 0 {
        // Drain the sequence cheaply and report total length
        let mut total = 0usize;
        while (seq.next_element::<IgnoredAny>()?).is_some() {
            total += 1;
        }
        return Ok((Vec::new(), Vec::new(), total));
    }

    let mut local_children: Vec<usize> = Vec::new();
    let mut local_indices: Vec<usize> = Vec::new();
    // Reserve conservatively to avoid huge allocations when cap is large
    let reserve = cap.min(4096);
    local_children.reserve(reserve);
    local_indices.reserve(reserve);

    let mut idx: usize = 0;
    let mut kept: usize = 0;

    // Always keep the first F items (up to cap)
    const F: usize = 3;
    let keep_first = F.min(cap);
    // Greedy head-of-middle: take about half of the remaining budget eagerly
    let mut greedy_remaining = (cap.saturating_sub(keep_first)) / 2;

    loop {
        if kept >= cap {
            // Budget exhausted: fast skip remainder
            while (seq.next_element::<IgnoredAny>()?).is_some() {
                idx = idx.saturating_add(1);
            }
            break;
        }

        if idx < keep_first {
            // Keep first few items unconditionally
            let cid = {
                let de = builder.seed();
                match seq.next_element_seed(de)? {
                    Some(c) => c,
                    None => break,
                }
            };
            local_children.push(cid);
            local_indices.push(idx);
            kept += 1;
            idx = idx.saturating_add(1);
            continue;
        }

        if greedy_remaining > 0 {
            let cid = {
                let de = builder.seed();
                match seq.next_element_seed(de)? {
                    Some(c) => c,
                    None => break,
                }
            };
            local_children.push(cid);
            local_indices.push(idx);
            kept += 1;
            greedy_remaining = greedy_remaining.saturating_sub(1);
            idx = idx.saturating_add(1);
            continue;
        }

        // Probabilistic accept for the remainder, deterministic and cap-independent
        if accept_index(idx as u64) {
            let cid = {
                let de = builder.seed();
                match seq.next_element_seed(de)? {
                    Some(c) => c,
                    None => break,
                }
            };
            local_children.push(cid);
            local_indices.push(idx);
            kept += 1;
        } else {
            // Skip cheaply without parsing the value
            if (seq.next_element::<IgnoredAny>()?).is_none() {
                break;
            }
        }
        idx = idx.saturating_add(1);
    }

    Ok((local_children, local_indices, idx))
}
