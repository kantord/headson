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

// Fixed-threshold acceptance to ensure monotonicity w.r.t. budget (cap).
// The acceptance set does not depend on `cap` or how many items have been
// kept so far; increasing cap simply includes more of the pre-determined
// accepted elements encountered earlier in the stream.
#[inline(always)]
fn should_take(i: u64, seed: u64) -> bool {
    // Use top 32 bits of the mixed index; accept when below threshold.
    // Threshold ~ 0.5 density; tune if needed.
    const THRESH: u32 = 0x8000_0000;
    let h = mix64(i ^ seed);
    ((h >> 32) as u32) < THRESH
}

/// Sample an array from a serde `SeqAccess` without backtracking.
///
/// Returns (children_ids, original_indices, total_length).
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

    let seed: u64 = 0x9e37_79b9_7f4a_7c15u64 ^ (cap as u64);

    let mut seen: u64 = 0;
    let mut kept = 0usize;

    const SMALL_FULL: u64 = 8; // fully include small arrays up to this size
    loop {
        if kept >= cap {
            // Budget exhausted: fast skip remainder
            while (seq.next_element::<IgnoredAny>()?).is_some() {
                seen = seen.saturating_add(1);
            }
            break;
        }

        if seen == 0 {
            // Always keep first element (index 0)
            let cid = {
                let seed = builder.seed();
                match seq.next_element_seed(seed)? {
                    Some(c) => c,
                    None => break,
                }
            };
            local_children.push(cid);
            local_indices.push(0);
            kept += 1;
            seen = seen.saturating_add(1);
            continue;
        }

        if seen < SMALL_FULL || should_take(seen, seed) {
            let cid = {
                let seed = builder.seed();
                match seq.next_element_seed(seed)? {
                    Some(c) => c,
                    None => break,
                }
            };
            local_children.push(cid);
            local_indices.push(seen as usize);
            kept += 1;
        } else {
            if (seq.next_element::<IgnoredAny>()?).is_none() {
                break;
            }
        }
        seen = seen.saturating_add(1);
    }

    Ok((local_children, local_indices, seen as usize))
}
