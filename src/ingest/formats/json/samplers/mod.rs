use serde::de::SeqAccess;

use crate::ingest::formats::json::builder::JsonTreeBuilder;
use crate::ingest::sampling::ArraySamplerKind;

#[derive(Debug)]
pub(crate) struct SampledArray {
    pub children: Vec<usize>,
    pub indices: Vec<usize>,
    pub total_len: usize,
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
                default::sample_stream(seq, builder, cap)
            }
            ArraySamplerKind::Head => head::sample_stream(seq, builder, cap),
            ArraySamplerKind::Tail => tail::sample_stream(seq, builder, cap),
        }
    }
}

mod default;
mod head;
mod tail;
