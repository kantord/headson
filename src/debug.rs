use serde::Serialize;
use std::ops::Not;

use crate::order::{ObjectType, PriorityOrder, ROOT_PQ_ID, RankedNode};
use crate::pruner::budget::FilesetSlots;
use crate::serialization::output::SlotStatsRecorder;

#[derive(Serialize)]
struct CountsDbg {
    total_nodes: usize,
    included: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    omitted_children: Option<usize>,
}

#[derive(Serialize)]
struct BudgetsDbg {
    #[serde(skip_serializing_if = "Option::is_none")]
    global: Option<BudgetEntryDbg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    per_slot: Option<BudgetEntryDbg>,
}

#[derive(Serialize, Copy, Clone)]
struct BudgetEntryDbg {
    kind: &'static str,
    cap: usize,
}

struct RenderDebugStats {
    output_stats: OutputStatsDbg,
    constrained_by: Vec<&'static str>,
}

fn budget_entry_dbg(b: Option<crate::Budget>) -> Option<BudgetEntryDbg> {
    b.map(|budget| BudgetEntryDbg {
        kind: match budget.kind {
            crate::BudgetKind::Bytes => "bytes",
            crate::BudgetKind::Chars => "chars",
            crate::BudgetKind::Lines => "lines",
        },
        cap: budget.cap,
    })
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    priority: Vec<PriorityNodeDbg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fileset: Option<Vec<FilesetItemDbg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    excluded_after_top_k: Option<Vec<PriorityNodeDbg>>,
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

#[derive(Serialize)]
struct PriorityNodeDbg {
    rank: usize,
    id: usize,
    score: u128,
    included: bool,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_in_object: Option<String>,
}

#[derive(Serialize)]
struct FilesetItemDbg {
    name: String,
    included: bool,
    content_included: usize,
    content_total: usize,
    rendered_empty: bool,
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

pub(crate) fn emit_render_debug(
    order_build: &crate::PriorityOrder,
    inclusion_flags: &[u32],
    render_set_id: u32,
    config: &crate::RenderConfig,
    budgets: crate::Budgets,
    top_k: usize,
) {
    let render_stats = collect_render_debug_stats(
        order_build,
        inclusion_flags,
        render_set_id,
        config,
        budgets,
    );
    let array_sampler = crate::ArraySamplerStrategy::Default;
    let dbg =
        crate::debug::build_render_debug_json(crate::debug::RenderDebugArgs {
            order: order_build,
            inclusion_flags,
            render_id: render_set_id,
            cfg: config,
            budgets,
            style: config.style,
            array_sampler,
            top_k,
            output_stats: render_stats.output_stats,
            constrained_by: render_stats.constrained_by,
        });
    #[allow(
        clippy::print_stderr,
        reason = "Debug mode emits JSON to stderr to aid troubleshooting"
    )]
    {
        eprintln!("{dbg}");
    }
}

fn collect_render_debug_stats(
    order_build: &crate::PriorityOrder,
    inclusion_flags: &[u32],
    render_set_id: u32,
    config: &crate::RenderConfig,
    budgets: crate::Budgets,
) -> RenderDebugStats {
    let mut no_color_cfg = config.clone();
    no_color_cfg.color_enabled = false;
    let slot_info = if budgets.per_slot.is_some() {
        FilesetSlots::new(order_build).or_else(|| {
            Some(FilesetSlots {
                map: vec![Some(0); order_build.total_nodes],
                count: 1,
                names: None,
            })
        })
    } else {
        None
    };
    let slot_count = slot_info
        .as_ref()
        .map(|slots| slots.count)
        .unwrap_or(0)
        .max(1);
    let recorder = budgets
        .per_slot
        .is_some()
        .then(|| SlotStatsRecorder::new(slot_count, budgets.measure_chars()));
    let (measured, slot_stats) =
        crate::serialization::render_from_render_set_with_slots(
            order_build,
            inclusion_flags,
            render_set_id,
            &no_color_cfg,
            slot_info.as_ref().map(|slots| slots.map.as_slice()),
            recorder,
        );
    let stats = crate::utils::measure::count_output_stats(
        &measured,
        budgets.measure_chars(),
    );
    let constrained_by = crate::pruner::budget::constrained_dimensions(
        budgets,
        &stats,
        slot_stats.as_deref(),
    );
    let out_stats = crate::debug::OutputStatsDbg {
        bytes: stats.bytes,
        chars: stats.chars,
        lines: stats.lines,
    };
    RenderDebugStats {
        output_stats: out_stats,
        constrained_by,
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

fn build_priority_dump(
    order: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    treat_atomic_as_string: bool,
) -> Vec<PriorityNodeDbg> {
    order
        .by_priority
        .iter()
        .enumerate()
        .map(|(rank, node_id)| {
            let pid = node_id.0;
            let node = &order.nodes[pid];
            let included = inclusion_flags
                .get(pid)
                .is_some_and(|flag| *flag == render_id);
            let atomic_token = match node {
                RankedNode::AtomicLeaf { token, .. } => Some(token.as_str()),
                _ => None,
            };
            let kind = kind_str(node, atomic_token, treat_atomic_as_string);
            let key =
                node.key_in_object().map(std::string::ToString::to_string);
            let score = order.scores.get(pid).copied().unwrap_or_default();
            PriorityNodeDbg {
                rank,
                id: pid,
                score,
                included,
                kind,
                key_in_object: key,
            }
        })
        .collect()
}

fn build_excluded_after_top_k(
    order: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    treat_atomic_as_string: bool,
    sample_size: usize,
) -> Vec<PriorityNodeDbg> {
    order
        .by_priority
        .iter()
        .enumerate()
        .filter_map(|(rank, node_id)| {
            let pid = node_id.0;
            let included = inclusion_flags
                .get(pid)
                .is_some_and(|flag| *flag == render_id);
            if included {
                return None;
            }
            let node = &order.nodes[pid];
            let atomic_token = match node {
                RankedNode::AtomicLeaf { token, .. } => Some(token.as_str()),
                _ => None,
            };
            let kind = kind_str(node, atomic_token, treat_atomic_as_string);
            let key =
                node.key_in_object().map(std::string::ToString::to_string);
            let score = order.scores.get(pid).copied().unwrap_or_default();
            Some(PriorityNodeDbg {
                rank,
                id: pid,
                score,
                included: false,
                kind,
                key_in_object: key,
            })
        })
        .take(sample_size)
        .collect()
}

#[allow(
    clippy::cognitive_complexity,
    reason = "Debug helper keeps traversal + accounting together for clarity"
)]
fn count_renderable_subtree(
    order: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    id: usize,
) -> (usize, usize) {
    let mut total = 0usize;
    let mut included = 0usize;
    let node = &order.nodes[id];
    let renderable = !matches!(node, RankedNode::LeafPart { .. });
    if renderable {
        total = total.saturating_add(1);
        if inclusion_flags
            .get(id)
            .is_some_and(|flag| *flag == render_id)
        {
            included = included.saturating_add(1);
        }
    }
    if let Some(children) = order.children.get(id) {
        for child in children {
            let (t, i) = count_renderable_subtree(
                order,
                inclusion_flags,
                render_id,
                child.0,
            );
            total = total.saturating_add(t);
            included = included.saturating_add(i);
        }
    }
    (total, included)
}

fn build_fileset_summary(
    order: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
) -> Option<Vec<FilesetItemDbg>> {
    if order.object_type.get(ROOT_PQ_ID) != Some(&ObjectType::Fileset) {
        return None;
    }
    let children = order
        .fileset_children
        .as_deref()
        .or_else(|| order.children.get(ROOT_PQ_ID).map(|v| &**v))
        .unwrap_or_default();
    let mut out = Vec::new();
    for child in children {
        let cid = child.0;
        let key = order.nodes[cid].key_in_object().unwrap_or("").to_string();
        let included_root = inclusion_flags
            .get(cid)
            .is_some_and(|flag| *flag == render_id);
        let (total, included) =
            count_renderable_subtree(order, inclusion_flags, render_id, cid);
        let rendered_empty = included_root && included == 0;
        out.push(FilesetItemDbg {
            name: key,
            included: included_root,
            content_included: included,
            content_total: total,
            rendered_empty,
        });
    }
    Some(out)
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
    let priority_dump = build_priority_dump(
        order,
        inclusion_flags,
        render_id,
        treat_atomic_as_string,
    );
    let excluded_after_top_k = build_excluded_after_top_k(
        order,
        inclusion_flags,
        render_id,
        treat_atomic_as_string,
        12,
    );
    let fileset = build_fileset_summary(order, inclusion_flags, render_id);
    let dump = DumpDbg {
        root,
        counts: CountsDbg {
            total_nodes: included,
            included,
            omitted_children: Some(omitted_children_sum),
        },
        template: template_str_for_root(order, cfg),
        budgets_effective: BudgetsDbg {
            global: budget_entry_dbg(budgets.global),
            per_slot: budget_entry_dbg(budgets.per_slot),
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
        priority: priority_dump,
        fileset,
        excluded_after_top_k: excluded_after_top_k
            .is_empty()
            .not()
            .then_some(excluded_after_top_k),
    };
    serde_json::to_string_pretty(&dump).unwrap()
}
