use serde::de::{IgnoredAny, SeqAccess};

use super::{JsonTreeBuilder, SampledArray};

fn parse_keep<'de, A>(
    seq: &mut A,
    builder: &JsonTreeBuilder,
    idx: usize,
    children: &mut Vec<usize>,
    indices: &mut Vec<usize>,
) -> Result<bool, A::Error>
where
    A: SeqAccess<'de>,
{
    let de = builder.seed();
    match seq.next_element_seed(de)? {
        Some(c) => {
            children.push(c);
            indices.push(idx);
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

// A minimal head sampler: keep the first N items only.
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

    let mut children: Vec<usize> = Vec::new();
    let mut indices: Vec<usize> = Vec::new();
    children.reserve(cap.min(4096));
    indices.reserve(cap.min(4096));

    let mut idx = 0usize;
    while children.len() < cap {
        if !parse_keep(seq, builder, idx, &mut children, &mut indices)? {
            return Ok(SampledArray {
                children,
                indices,
                total_len: idx,
            });
        }
        idx = idx.saturating_add(1);
    }
    while skip_one(seq)? {
        idx = idx.saturating_add(1);
    }

    Ok(SampledArray {
        children,
        indices,
        total_len: idx,
    })
}
