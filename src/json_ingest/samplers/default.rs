use serde::de::{IgnoredAny, SeqAccess};

use super::{JsonTreeBuilder, SampledArray};

// Tunable sampling constants for the default strategy.
const RANDOM_ACCEPT_SEED: u64 = 0x9e37_79b9_7f4a_7c15;
// ~50% acceptance to thin remaining elements in the random phase.
const RANDOM_ACCEPT_THRESHOLD: u32 = 0x8000_0000;
// Keep a small, fixed number of items from the head before greedy/random phases.
const KEEP_FIRST_COUNT: usize = 3;
// Take roughly half of the remaining capacity greedily after the first items.
const GREEDY_PORTION_DIVISOR: usize = 2;

struct PhaseState {
    idx: usize,
    kept: usize,
}

struct SampleAccumulator<'a> {
    children: &'a mut Vec<usize>,
    indices: &'a mut Vec<usize>,
}

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

fn parse_keep<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    idx: usize,
    out: &mut SampleAccumulator<'_>,
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

fn skip_one<'de, A>(seq: &mut A) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    Ok(seq.next_element::<IgnoredAny>()?.is_some())
}

fn phase_keep_first<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
    keep_first: usize,
    state: &mut PhaseState,
    out: &mut SampleAccumulator<'_>,
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

fn phase_greedy<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
    greedy_remaining: &mut usize,
    state: &mut PhaseState,
    out: &mut SampleAccumulator<'_>,
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

fn phase_random<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    cap: usize,
    state: &mut PhaseState,
    out: &mut SampleAccumulator<'_>,
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
    let reserve = cap.min(4096);
    local_children.reserve(reserve);
    local_indices.reserve(reserve);

    let mut state = PhaseState { idx: 0, kept: 0 };

    let keep_first = KEEP_FIRST_COUNT.min(cap);
    let mut greedy_remaining =
        (cap.saturating_sub(keep_first)) / GREEDY_PORTION_DIVISOR;

    if phase_keep_first(
        seq,
        builder,
        cap,
        keep_first,
        &mut state,
        &mut SampleAccumulator {
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
    if phase_greedy(
        seq,
        builder,
        cap,
        &mut greedy_remaining,
        &mut state,
        &mut SampleAccumulator {
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
    phase_random(
        seq,
        builder,
        cap,
        &mut state,
        &mut SampleAccumulator {
            children: &mut local_children,
            indices: &mut local_indices,
        },
    )?;

    while skip_one(seq)? {
        state.idx = state.idx.saturating_add(1);
    }

    Ok(SampledArray {
        children: local_children,
        indices: local_indices,
        total_len: state.idx,
    })
}
