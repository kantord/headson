use serde::Serialize;

use crate::order::{ObjectType, PriorityOrder, ROOT_PQ_ID, RankedNode};

#[derive(Serialize)]
struct CountsDbg {
    total_nodes: usize,
    included: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    omitted_children: Option<usize>,
}

#[derive(Serialize)]
struct BudgetsDbg {
    bytes: Option<usize>,
    chars: Option<usize>,
    lines: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct DumpDbg<'a> {
    root: NodeDbg,
    counts: CountsDbg,
    template: &'a str,
    budgets_effective: BudgetsDbg,
    selection: SelectionDbg,
    renderer: RendererDbg<'a>,
    output_stats: OutputStatsDbg,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    constrained_by: Vec<&'a str>,
}

#[derive(Serialize)]
struct SelectionDbg {
    top_k: usize,
}

#[derive(Serialize)]
struct RendererDbg<'a> {
    template: &'a str,
    style: &'a str,
    prefer_tail_arrays: bool,
    array_sampler: &'a str,
}

#[derive(Serialize, Default)]
pub(crate) struct OutputStatsDbg {
    pub bytes: usize,
    pub chars: usize,
    pub lines: usize,
}

pub(crate) struct RenderDebugArgs<'a> {
    pub order: &'a PriorityOrder,
    pub inclusion_flags: &'a [u32],
    pub render_id: u32,
    pub cfg: &'a crate::RenderConfig,
    pub budgets: crate::Budgets,
    pub style: crate::serialization::types::Style,
    pub array_sampler: crate::ArraySamplerStrategy,
    pub top_k: usize,
    pub output_stats: OutputStatsDbg,
    pub constrained_by: Vec<&'a str>,
}

#[derive(Serialize)]
struct MetricsDbg {
    #[serde(skip_serializing_if = "Option::is_none")]
    array_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_len: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    string_len: Option<usize>,
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde skip_serializing_if expects a &T predicate signature"
    )]
    #[serde(skip_serializing_if = "is_false")]
    string_truncated: bool,
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if expects a &T predicate signature"
)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize)]
struct NodeDbg {
    id: usize,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_in_object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index_in_parent_array: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    string_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atomic_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fileset_root: Option<bool>,
    metrics: MetricsDbg,
    #[serde(skip_serializing_if = "Option::is_none")]
    omitted_before: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    omitted_after: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    gaps: Vec<GapDbg>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<NodeDbg>,
}

#[derive(Serialize)]
struct GapDbg {
    before_child_index: usize,
    omitted_count: usize,
}

fn template_str_for_root(
    order: &PriorityOrder,
    cfg: &crate::RenderConfig,
) -> &'static str {
    use crate::serialization::types::OutputTemplate as T;
    if order.object_type.get(ROOT_PQ_ID) == Some(&ObjectType::Fileset) {
        // In filesets root, per-file templates may vary under Auto; report "auto".
        return "auto";
    }
    match cfg.template {
        T::Json => "json",
        T::Pseudo => "pseudo",
        T::Js => "js",
        T::Yaml => "yaml",
        T::Text => "text",
        T::Code => "code",
        T::Auto => match cfg.style {
            crate::serialization::types::Style::Strict => "json",
            crate::serialization::types::Style::Default => "pseudo",
            crate::serialization::types::Style::Detailed => "js",
        },
    }
}

fn style_str(s: crate::serialization::types::Style) -> &'static str {
    match s {
        crate::serialization::types::Style::Strict => "strict",
        crate::serialization::types::Style::Default => "default",
        crate::serialization::types::Style::Detailed => "detailed",
    }
}

fn kind_str(
    node: &RankedNode,
    atomic_token: Option<&str>,
    treat_atomic_as_string: bool,
) -> String {
    match node {
        RankedNode::Array { .. } => "array".into(),
        RankedNode::Object { .. } => "object".into(),
        RankedNode::SplittableLeaf { .. } => "string".into(),
        RankedNode::LeafPart { .. } => "string-part".into(),
        RankedNode::AtomicLeaf { .. } => {
            if treat_atomic_as_string {
                // Under text template, atomic tokens represent whole lines; treat as string.
                "string".into()
            } else {
                match atomic_token {
                    Some("null") => "null".into(),
                    Some("true") | Some("false") => "bool".into(),
                    Some(_) => "number".into(),
                    None => "atomic".into(),
                }
            }
        }
    }
}

fn make_metrics(order: &PriorityOrder, id: usize) -> MetricsDbg {
    let m = &order.metrics[id];
    MetricsDbg {
        array_len: m.array_len,
        object_len: m.object_len,
        string_len: m.string_len,
        string_truncated: m.string_truncated,
    }
}

fn string_preview(value: &str) -> String {
    // Show a small, grapheme-aware prefix to aid debugging.
    let prefix = crate::utils::text::take_n_graphemes(value, 32);
    if prefix.len() < value.len() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

struct BuildCtx<'a> {
    order: &'a PriorityOrder,
    inclusion_flags: &'a [u32],
    render_id: u32,
    include_count: &'a mut usize,
    omitted_children_sum: &'a mut usize,
    treat_atomic_as_string: bool,
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "Pruned tree emission keeps branching in one place for clarity"
)]
fn build_node(ctx: &mut BuildCtx<'_>, id: usize) -> NodeDbg {
    let order = ctx.order;
    let inclusion_flags = ctx.inclusion_flags;
    let render_id = ctx.render_id;
    let treat_atomic_as_string = ctx.treat_atomic_as_string;
    let rn = &order.nodes[id];
    let key_in_object =
        rn.key_in_object().map(std::string::ToString::to_string);
    let index_in_parent_array = order.index_in_parent_array[id];
    let fileset_root = if id == ROOT_PQ_ID
        && order.object_type.get(id) == Some(&ObjectType::Fileset)
    {
        Some(true)
    } else {
        None
    };

    // Count only renderable nodes (skip string parts).
    let renderable = !matches!(rn, RankedNode::LeafPart { .. });
    if renderable {
        *ctx.include_count += 1;
    }

    // Leaf handling and children traversal
    #[derive(Default)]
    struct Built {
        string_preview_opt: Option<String>,
        atomic_token_opt: Option<String>,
        children: Vec<NodeDbg>,
        kept_indices: Vec<usize>,
    }

    let Built {
        string_preview_opt,
        atomic_token_opt,
        children,
        mut kept_indices,
    } = match rn {
        RankedNode::SplittableLeaf { value, .. } => Built {
            string_preview_opt: Some(string_preview(value)),
            ..Default::default()
        },
        RankedNode::AtomicLeaf { token, .. } => Built {
            atomic_token_opt: Some(token.clone()),
            ..Default::default()
        },
        RankedNode::LeafPart { .. } => Built::default(),
        RankedNode::Array { .. } | RankedNode::Object { .. } => {
            let mut kids = Vec::new();
            let mut idxs = Vec::new();
            if let Some(ch) = order.children.get(id) {
                for (i, &cid) in ch.iter().enumerate() {
                    let cid_usize = cid.0;
                    if inclusion_flags[cid_usize] != render_id {
                        continue;
                    }
                    // Skip synthetic string parts in debug tree to match render
                    if matches!(
                        order.nodes[cid_usize],
                        RankedNode::LeafPart { .. }
                    ) {
                        continue;
                    }
                    let orig_index = order
                        .index_in_parent_array
                        .get(cid_usize)
                        .copied()
                        .flatten()
                        .unwrap_or(i);
                    idxs.push(orig_index);
                    kids.push(build_node(ctx, cid_usize));
                }
            }
            Built {
                children: kids,
                kept_indices: idxs,
                ..Default::default()
            }
        }
    };

    let atomic_token_ref = atomic_token_opt.as_deref();
    // Omission info
    let mut omitted_before = None;
    let mut omitted_after = None;
    let mut gaps: Vec<GapDbg> = Vec::new();
    if matches!(rn, RankedNode::Array { .. } | RankedNode::Object { .. }) {
        let total_opt = match &order.metrics[id] {
            m if m.array_len.is_some() => m.array_len,
            m if m.object_len.is_some() => m.object_len,
            _ => None,
        };
        if let (Some(total), true) = (total_opt, !kept_indices.is_empty()) {
            kept_indices.sort_unstable();
            let first = kept_indices[0];
            let last = kept_indices.last().copied().unwrap_or(first);
            let before = first;
            let after = total.saturating_sub(1).saturating_sub(last);
            if before > 0 {
                omitted_before = Some(before);
            }
            if after > 0 {
                omitted_after = Some(after);
            }
            let mut prev = first;
            for (ci, &cur) in kept_indices.iter().enumerate().skip(1) {
                let gap = cur.saturating_sub(prev).saturating_sub(1);
                if gap > 0 {
                    gaps.push(GapDbg {
                        before_child_index: ci,
                        omitted_count: gap,
                    });
                }
                prev = cur;
            }
            let kept_count = kept_indices.len();
            let omitted = total.saturating_sub(kept_count);
            *ctx.omitted_children_sum =
                ctx.omitted_children_sum.saturating_add(omitted);
        }
    }

    NodeDbg {
        id,
        kind: kind_str(rn, atomic_token_ref, treat_atomic_as_string),
        key_in_object,
        index_in_parent_array,
        string_preview: string_preview_opt,
        atomic_token: atomic_token_opt,
        fileset_root,
        metrics: make_metrics(order, id),
        omitted_before,
        omitted_after,
        gaps,
        children,
    }
}

#[allow(
    clippy::unwrap_used,
    reason = "Debug mode should panic on serialization errors to surface bugs"
)]
pub(crate) fn build_render_debug_json(args: RenderDebugArgs) -> String {
    let RenderDebugArgs {
        order,
        inclusion_flags,
        render_id,
        cfg,
        budgets,
        style,
        array_sampler,
        top_k,
        output_stats,
        constrained_by,
    } = args;
    let mut included = 0usize;
    let mut omitted_children_sum: usize = 0;
    let root_is_fileset =
        order.object_type.get(ROOT_PQ_ID) == Some(&ObjectType::Fileset);
    let treat_atomic_as_string =
        matches!(
            cfg.template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) || (matches!(cfg.template, crate::OutputTemplate::Auto)
            && root_is_fileset);
    let mut ctx = BuildCtx {
        order,
        inclusion_flags,
        render_id,
        include_count: &mut included,
        omitted_children_sum: &mut omitted_children_sum,
        treat_atomic_as_string,
    };
    let root = build_node(&mut ctx, ROOT_PQ_ID);
    let dump = DumpDbg {
        root,
        counts: CountsDbg {
            total_nodes: included,
            included,
            omitted_children: Some(omitted_children_sum),
        },
        template: template_str_for_root(order, cfg),
        budgets_effective: BudgetsDbg {
            bytes: budgets.byte_budget,
            chars: budgets.char_budget,
            lines: budgets.line_budget,
        },
        selection: SelectionDbg { top_k },
        renderer: RendererDbg {
            template: template_str_for_root(order, cfg),
            style: style_str(style),
            prefer_tail_arrays: cfg.prefer_tail_arrays,
            array_sampler: match array_sampler {
                crate::ArraySamplerStrategy::Default => "default",
                crate::ArraySamplerStrategy::Head => "head",
                crate::ArraySamplerStrategy::Tail => "tail",
            },
        },
        output_stats,
        constrained_by,
    };
    serde_json::to_string_pretty(&dump).unwrap()
}
