use crate::order::ObjectType;
use crate::order::{NodeKind, PriorityOrder, ROOT_PQ_ID, RankedNode};
pub mod color;
mod fileset;
pub mod output;
pub mod templates;
pub mod types;
use self::templates::{ArrayCtx, ObjectCtx, render_array, render_object};
use crate::serialization::output::Out;

type ArrayChildPair = (usize, (NodeKind, String));
type ObjectChildPair = (usize, (String, String));

pub(crate) struct RenderScope<'a> {
    // Priority-ordered view of the parsed JSON tree.
    order: &'a PriorityOrder,
    // Per-node inclusion flag: a node is included in the current render attempt
    // when inclusion_flags[node_id] == render_set_id. This avoids clearing the
    // vector between render attempts by bumping render_set_id each time.
    inclusion_flags: &'a [u32],
    // Identifier for the current inclusion set (render pass).
    render_set_id: u32,
    // Rendering configuration (template, whitespace, etc.).
    config: &'a crate::RenderConfig,
}

impl<'a> RenderScope<'a> {
    fn push_array_child_line(
        &self,
        out: &mut Vec<ArrayChildPair>,
        index: usize,
        child_kind: NodeKind,
        _depth: usize,
        rendered: String,
    ) {
        // Defer indentation concerns to templates; store kind + rendered.
        out.push((index, (child_kind, rendered)));
    }

    fn count_kept_children(&self, id: usize) -> usize {
        if let Some(kids) = self.order.children.get(id) {
            let mut kept = 0usize;
            for &cid in kids {
                if self.inclusion_flags[cid.0] == self.render_set_id {
                    kept += 1;
                }
            }
            kept
        } else {
            0
        }
    }

    fn omitted_for_string(&self, id: usize, kept: usize) -> Option<usize> {
        let m = &self.order.metrics[id];
        if let Some(orig) = m.string_len {
            if orig > kept {
                return Some(orig - kept);
            }
            if m.string_truncated {
                return Some(1);
            }
            None
        } else if m.string_truncated {
            Some(1)
        } else {
            None
        }
    }

    fn omitted_for(&self, id: usize, kept: usize) -> Option<usize> {
        match &self.order.nodes[id] {
            RankedNode::Array { .. } => {
                self.order.metrics[id].array_len.and_then(|orig| {
                    if orig > kept { Some(orig - kept) } else { None }
                })
            }
            RankedNode::Object { .. } => {
                self.order.metrics[id].object_len.and_then(|orig| {
                    if orig > kept { Some(orig - kept) } else { None }
                })
            }
            RankedNode::SplittableLeaf { .. } => {
                self.omitted_for_string(id, kept)
            }
            RankedNode::AtomicLeaf { .. } | RankedNode::LeafPart { .. } => {
                None
            }
        }
    }

    fn write_array(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
    ) {
        let config = self.config;
        let (children_pairs, kept) = self.gather_array_children(id, depth);
        let omitted = self.omitted_for(id, kept).unwrap_or(0);
        let ctx = ArrayCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            inline_open: inline,
            omitted_at_start: config.prefer_tail_arrays,
        };
        render_array(config.template, &ctx, out)
    }

    fn write_object(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
    ) {
        let config = self.config;
        if let Some(rendered) = self.try_render_fileset_root(id, depth) {
            out.push_str(&rendered);
            return;
        }
        let (children_pairs, kept) = self.gather_object_children(id, depth);
        let omitted = self.omitted_for(id, kept).unwrap_or(0);
        let ctx = ObjectCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            inline_open: inline,
            space: &config.space,
            fileset_root: id == ROOT_PQ_ID
                && self.order.object_type.get(id)
                    == Some(&ObjectType::Fileset),
        };
        render_object(config.template, &ctx, out)
    }

    fn serialize_string(&mut self, id: usize) -> String {
        let kept = self.count_kept_children(id);
        let omitted = self.omitted_for(id, kept).unwrap_or(0);
        let full: &str = match &self.order.nodes[id] {
            RankedNode::SplittableLeaf { value, .. } => value.as_str(),
            _ => unreachable!(
                "serialize_string called for non-string node: id={id}"
            ),
        };
        if omitted == 0 {
            crate::utils::json::json_string(full)
        } else {
            let prefix = crate::utils::text::take_n_graphemes(full, kept);
            let truncated = format!("{prefix}…");
            crate::utils::json::json_string(&truncated)
        }
    }

    fn serialize_atomic(&self, id: usize) -> String {
        match &self.order.nodes[id] {
            RankedNode::AtomicLeaf { token, .. } => token.clone(),
            _ => unreachable!("atomic leaf without token: id={id}"),
        }
    }

    fn write_node(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
    ) {
        match &self.order.nodes[id] {
            RankedNode::Array { .. } => {
                self.write_array(id, depth, inline, out)
            }
            RankedNode::Object { .. } => {
                self.write_object(id, depth, inline, out)
            }
            RankedNode::SplittableLeaf { .. } => {
                let s = self.serialize_string(id);
                out.push_string_literal(&s);
            }
            RankedNode::AtomicLeaf { .. } => {
                let s = self.serialize_atomic(id);
                out.push_str(&s);
            }
            RankedNode::LeafPart { .. } => {
                unreachable!("string part should not be rendered")
            }
        }
    }

    fn gather_array_children(
        &mut self,
        id: usize,
        depth: usize,
    ) -> (Vec<ArrayChildPair>, usize) {
        let mut children_pairs: Vec<ArrayChildPair> = Vec::new();
        let mut kept = 0usize;
        if let Some(children_ids) = self.order.children.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                kept += 1;
                let child_kind = self.order.nodes[child_id.0].display_kind();
                let rendered =
                    self.render_node_to_string(child_id.0, depth + 1, false);
                let orig_index = self
                    .order
                    .index_in_parent_array
                    .get(child_id.0)
                    .and_then(|o| *o)
                    .unwrap_or(i);
                self.push_array_child_line(
                    &mut children_pairs,
                    orig_index,
                    child_kind,
                    depth,
                    rendered,
                );
            }
        }
        (children_pairs, kept)
    }

    fn gather_object_children(
        &mut self,
        id: usize,
        depth: usize,
    ) -> (Vec<ObjectChildPair>, usize) {
        let mut children_pairs: Vec<ObjectChildPair> = Vec::new();
        let mut kept = 0usize;
        if let Some(children_ids) = self.order.children.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                kept += 1;
                let child = &self.order.nodes[child_id.0];
                let raw_key = child.key_in_object().unwrap_or("");
                let key = crate::utils::json::json_string(raw_key);
                let val =
                    self.render_node_to_string(child_id.0, depth + 1, true);
                children_pairs.push((i, (key, val)));
            }
        }
        (children_pairs, kept)
    }

    fn render_node_to_string(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
    ) -> String {
        match &self.order.nodes[id] {
            RankedNode::Array { .. } => {
                let mut s = String::new();
                let mut ow = Out::new(
                    &mut s,
                    &self.config.newline,
                    &self.config.indent_unit,
                    self.config.color_enabled,
                );
                self.write_array(id, depth, inline, &mut ow);
                s
            }
            RankedNode::Object { .. } => {
                let mut s = String::new();
                let mut ow = Out::new(
                    &mut s,
                    &self.config.newline,
                    &self.config.indent_unit,
                    self.config.color_enabled,
                );
                self.write_object(id, depth, inline, &mut ow);
                s
            }
            RankedNode::SplittableLeaf { .. } => self.serialize_string(id),
            RankedNode::AtomicLeaf { .. } => self.serialize_atomic(id),
            RankedNode::LeafPart { .. } => {
                unreachable!("string part not rendered")
            }
        }
    }
}

/// Prepare a render set by including the first `top_k` nodes by priority
/// and all of their ancestors so the output remains structurally valid.
pub fn prepare_render_set_top_k_and_ancestors(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
) {
    if inclusion_flags.len() < order_build.total_nodes {
        inclusion_flags.resize(order_build.total_nodes, 0);
    }
    let k = top_k.min(order_build.total_nodes);
    crate::utils::graph::mark_top_k_and_ancestors(
        order_build,
        k,
        inclusion_flags,
        render_id,
    );
}

/// Render using a previously prepared render set (inclusion flags matching `render_id`).
pub fn render_from_render_set(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    let root_id = ROOT_PQ_ID;
    let mut scope = RenderScope {
        order: order_build,
        inclusion_flags,
        render_set_id: render_id,
        config,
    };
    let mut s = String::new();
    let mut out = Out::new(
        &mut s,
        &config.newline,
        &config.indent_unit,
        config.color_enabled,
    );
    scope.write_node(root_id, 0, false, &mut out);
    s
}

/// Convenience: prepare the render set for `top_k` nodes and render in one call.
pub fn render_top_k(
    order_build: &PriorityOrder,
    top_k: usize,
    inclusion_flags: &mut Vec<u32>,
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    prepare_render_set_top_k_and_ancestors(
        order_build,
        top_k,
        inclusion_flags,
        render_id,
    );
    render_from_render_set(order_build, inclusion_flags, render_id, config)
}

//

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::build_order;
    use insta::assert_snapshot;

    fn assert_yaml_valid(s: &str) {
        let _: serde_yaml::Value =
            serde_yaml::from_str(s).expect("YAML parse failed (validation)");
    }

    #[test]
    fn arena_render_empty_array() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Auto,
                color_enabled: false,
            },
        );
        assert_snapshot!("arena_render_empty", out);
    }

    #[test]
    fn newline_detection_crlf_array_child() {
        // Ensure we exercise the render_has_newline branch that checks
        // arbitrary newline sequences (e.g., "\r\n") via s.contains(nl).
        let arena = crate::json_ingest::build_json_tree_arena(
            "[{\"a\":1,\"b\":2}]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            usize::MAX,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                // Use CRLF to force the contains(nl) path.
                newline: "\r\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Auto,
                color_enabled: false,
            },
        );
        // Sanity: output should contain CRLF newlines and render the object child across lines.
        assert!(
            out.contains("\r\n"),
            "expected CRLF newlines in output: {out:?}"
        );
        assert!(out.starts_with("["));
    }

    #[test]
    fn arena_render_single_string_array() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[\"ab\"]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Auto,
                color_enabled: false,
            },
        );
        assert_snapshot!("arena_render_single", out);
    }

    #[test]
    fn array_omitted_markers_pseudo_head_and_tail() {
        // Force sampling to keep only a subset so omitted > 0.
        let cfg_prio = crate::PriorityConfig {
            max_string_graphemes: usize::MAX,
            array_max_items: 1,
            prefer_tail_arrays: false,
            array_bias: crate::ArrayBias::HeadMidTail,
            array_sampler: crate::ArraySamplerStrategy::Default,
        };
        let arena =
            crate::json_ingest::build_json_tree_arena("[1,2,3]", &cfg_prio)
                .unwrap();
        let build = build_order(&arena, &cfg_prio).unwrap();
        let mut marks = vec![0u32; build.total_nodes];

        // Head preference: omitted marker after items.
        let out_head = render_top_k(
            &build,
            2,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Pseudo,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_snapshot!("array_omitted_pseudo_head", out_head);

        // Tail preference: omitted marker before items (with comma).
        let out_tail = render_top_k(
            &build,
            2,
            &mut marks,
            2,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Pseudo,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: true,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_snapshot!("array_omitted_pseudo_tail", out_tail);
    }

    #[test]
    fn array_omitted_markers_js_head_and_tail() {
        let cfg_prio = crate::PriorityConfig {
            max_string_graphemes: usize::MAX,
            array_max_items: 1,
            prefer_tail_arrays: false,
            array_bias: crate::ArrayBias::HeadMidTail,
            array_sampler: crate::ArraySamplerStrategy::Default,
        };
        let arena =
            crate::json_ingest::build_json_tree_arena("[1,2,3]", &cfg_prio)
                .unwrap();
        let build = build_order(&arena, &cfg_prio).unwrap();
        let mut marks = vec![0u32; build.total_nodes];

        let out_head = render_top_k(
            &build,
            2,
            &mut marks,
            3,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Js,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_snapshot!("array_omitted_js_head", out_head);

        let out_tail = render_top_k(
            &build,
            2,
            &mut marks,
            4,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Js,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: true,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_snapshot!("array_omitted_js_tail", out_tail);
    }

    #[test]
    fn array_omitted_markers_yaml_head_and_tail() {
        let cfg_prio = crate::PriorityConfig {
            max_string_graphemes: usize::MAX,
            array_max_items: 1,
            prefer_tail_arrays: false,
            array_bias: crate::ArrayBias::HeadMidTail,
            array_sampler: crate::ArraySamplerStrategy::Default,
        };
        let arena =
            crate::json_ingest::build_json_tree_arena("[1,2,3]", &cfg_prio)
                .unwrap();
        let build = build_order(&arena, &cfg_prio).unwrap();
        let mut marks = vec![0u32; build.total_nodes];

        let out_head = render_top_k(
            &build,
            2,
            &mut marks,
            11,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out_head);
        assert_snapshot!("array_omitted_yaml_head", out_head);

        let out_tail = render_top_k(
            &build,
            2,
            &mut marks,
            12,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: true,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out_tail);
        assert_snapshot!("array_omitted_yaml_tail", out_tail);
    }

    #[test]
    fn arena_render_empty_array_yaml() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            10,
            &mut marks,
            21,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Auto,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out);
        assert_snapshot!("arena_render_empty_yaml", out);
    }

    #[test]
    fn arena_render_single_string_array_yaml() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[\"ab\"]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            10,
            &mut marks,
            22,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Auto,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out);
        assert_snapshot!("arena_render_single_yaml", out);
    }

    #[test]
    fn inline_open_array_in_object_yaml() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "{\"a\":[1,2,3]}",
            &crate::PriorityConfig::new(usize::MAX, 2),
        )
        .unwrap();
        let build =
            build_order(&arena, &crate::PriorityConfig::new(usize::MAX, 2))
                .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            4,
            &mut marks,
            23,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out);
        assert_snapshot!("inline_open_array_in_object_yaml", out);
    }

    #[test]
    fn array_internal_gaps_yaml() {
        let ctx = mk_gap_ctx();
        let mut s = String::new();
        let mut outw =
            crate::serialization::output::Out::new(&mut s, "\n", "  ", false);
        super::templates::render_array(
            crate::OutputTemplate::Yaml,
            &ctx,
            &mut outw,
        );
        let out = s;
        assert_yaml_valid(&out);
        assert_snapshot!("array_internal_gaps_yaml", out);
    }

    #[test]
    #[allow(
        clippy::cognitive_complexity,
        reason = "Aggregated YAML quoting cases in one test to reuse setup."
    )]
    fn yaml_key_and_scalar_quoting() {
        // Keys and values that exercise YAML quoting heuristics.
        let json = "{\n            \"true\": 1,\n            \"010\": \"010\",\n            \"-dash\": \"ok\",\n            \"normal\": \"simple\",\n            \"a:b\": \"a:b\",\n            \" spaced \": \" spaced \",\n            \"reserved\": \"yes\",\n            \"multiline\": \"line1\\nline2\"\n        }";
        let arena = crate::json_ingest::build_json_tree_arena(
            json,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            usize::MAX,
            &mut marks,
            27,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out);
        // Unquoted safe key
        assert!(
            out.contains("normal: simple"),
            "expected unquoted normal key/value: {out:?}"
        );
        // Quoted key starting with digit and quoted numeric-looking value
        assert!(
            out.contains("\"010\": \"010\""),
            "expected quoted numeric-like key and value: {out:?}"
        );
        // Quoted key with punctuation ':' and quoted value with ':'
        assert!(
            out.contains("\"a:b\": \"a:b\""),
            "expected quoted punctuated key/value: {out:?}"
        );
        // Quoted key/value with outer whitespace
        assert!(
            out.contains("\" spaced \": \" spaced \""),
            "expected quotes for outer whitespace: {out:?}"
        );
        // Reserved word value quoted
        assert!(
            out.contains("reserved: \"yes\""),
            "expected reserved word value quoted: {out:?}"
        );
        // Multiline string stays quoted and appears on a single line token here
        assert!(
            out.contains("multiline: \"line1\\nline2\""),
            "expected JSON-escaped newline token for strings: {out:?}"
        );
        // Key 'true' must be quoted to avoid YAML boolean
        assert!(
            out.contains("\"true\": 1"),
            "expected quoted boolean-like key: {out:?}"
        );
    }

    #[test]
    fn string_parts_never_rendered_but_affect_truncation() {
        // Build a long string: the string node itself is SplittableLeaf; the
        // builder also creates LeafPart children used only for priority.
        let arena = crate::json_ingest::build_json_tree_arena(
            "\"abcdefghij\"",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        // Include the root string node plus 5 grapheme parts (total top_k = 1 + 5).
        let out = render_top_k(
            &build,
            6,
            &mut marks,
            99,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "".to_string(),
                space: " ".to_string(),
                newline: "".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        // Expect the first 5 characters plus an ellipsis, as a valid JSON string literal.
        assert_eq!(out, "\"abcde…\"");
    }

    #[test]
    fn yaml_array_of_objects_indentation() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "[{\"a\":1,\"b\":2},{\"x\":3}]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            usize::MAX,
            &mut marks,
            28,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Yaml,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_yaml_valid(&out);
        // Expect dash-prefixed first line and continued indentation for following lines
        assert!(
            out.contains("- a: 1") || out.contains("-   a: 1"),
            "expected list dash with first object line: {out:?}"
        );
        assert!(
            out.contains("  b: 2"),
            "expected subsequent object key indented: {out:?}"
        );
    }

    #[test]
    fn omitted_for_atomic_returns_none() {
        // Single atomic value as input (number), root is AtomicLeaf.
        let arena = crate::json_ingest::build_json_tree_arena(
            "1",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let render_id = 7u32;
        // Mark the root included for this render set.
        marks[crate::order::ROOT_PQ_ID] = render_id;
        let cfg = crate::RenderConfig {
            template: crate::OutputTemplate::Json,
            indent_unit: "".to_string(),
            space: " ".to_string(),
            newline: "".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
        };
        let scope = RenderScope {
            order: &build,
            inclusion_flags: &marks,
            render_set_id: render_id,
            config: &cfg,
        };
        // Atomic leaves never report omitted counts.
        let none = scope.omitted_for(crate::order::ROOT_PQ_ID, 0);
        assert!(none.is_none());
    }

    #[test]
    fn inline_open_array_in_object_json() {
        let arena = crate::json_ingest::build_json_tree_arena(
            "{\"a\":[1,2,3]}",
            &crate::PriorityConfig::new(usize::MAX, 2),
        )
        .unwrap();
        let build =
            build_order(&arena, &crate::PriorityConfig::new(usize::MAX, 2))
                .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_top_k(
            &build,
            4,
            &mut marks,
            5,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Off,
                color_enabled: false,
            },
        );
        assert_snapshot!("inline_open_array_in_object_json", out);
    }

    #[test]
    fn arena_render_object_partial_js() {
        // Object with three properties; render top_k small so only one child is kept.
        let arena = crate::json_ingest::build_json_tree_arena(
            "{\"a\":1,\"b\":2,\"c\":3}",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = build_order(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut flags = vec![0u32; build.total_nodes];
        // top_k=2 → root object + first property
        let out = render_top_k(
            &build,
            2,
            &mut flags,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Js,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
                prefer_tail_arrays: false,
                color_mode: crate::ColorMode::Auto,
                color_enabled: false,
            },
        );
        // Should be a valid JS object with one property and an omitted summary.
        assert!(out.starts_with("{\n"));
        assert!(
            out.contains("/* 2 more properties */"),
            "missing omitted summary: {out:?}"
        );
        assert!(
            out.contains("\"a\": 1")
                || out.contains("\"b\": 2")
                || out.contains("\"c\": 3")
        );
    }

    fn mk_gap_ctx() -> super::templates::ArrayCtx {
        super::templates::ArrayCtx {
            children: vec![
                (0, (crate::order::NodeKind::Number, "1".to_string())),
                (3, (crate::order::NodeKind::Number, "2".to_string())),
                (5, (crate::order::NodeKind::Number, "3".to_string())),
            ],
            children_len: 3,
            omitted: 0,
            depth: 0,
            inline_open: false,
            omitted_at_start: false,
        }
    }

    fn assert_contains_all(out: &str, needles: &[&str]) {
        needles.iter().for_each(|n| assert!(out.contains(n)));
    }

    #[test]
    fn array_internal_gaps_pseudo() {
        let ctx = mk_gap_ctx();
        let mut s = String::new();
        let mut outw =
            crate::serialization::output::Out::new(&mut s, "\n", "  ", false);
        super::templates::render_array(
            crate::OutputTemplate::Pseudo,
            &ctx,
            &mut outw,
        );
        let out = s;
        assert_contains_all(
            &out,
            &["[\n", "\n  1,", "\n  …\n", "\n  2,", "\n  3\n"],
        );
    }

    #[test]
    fn array_internal_gaps_js() {
        let ctx = mk_gap_ctx();
        let mut s = String::new();
        let mut outw =
            crate::serialization::output::Out::new(&mut s, "\n", "  ", false);
        super::templates::render_array(
            crate::OutputTemplate::Js,
            &ctx,
            &mut outw,
        );
        let out = s;
        assert!(out.contains("/* 2 more items */"));
        assert!(out.contains("/* 1 more items */"));
    }
}
