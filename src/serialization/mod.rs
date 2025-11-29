use crate::order::ObjectType;
use crate::order::{NodeId, NodeKind, PriorityOrder, ROOT_PQ_ID, RankedNode};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
pub mod color;
mod fileset;
mod highlight;
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
    // Optional global width for line number alignment when enabled.
    line_number_width: Option<usize>,
    code_highlight_cache: HashMap<usize, Arc<Vec<String>>>,
    grep_highlight: Option<Regex>,
}

impl<'a> RenderScope<'a> {
    fn is_text_omission_line(
        style: crate::serialization::types::Style,
        raw: &str,
    ) -> bool {
        let s = raw.trim();
        if s.is_empty() {
            return true;
        }
        match style {
            // Default: a single omission marker
            crate::serialization::types::Style::Default => s == "…",
            // Detailed: either single marker or "… N more lines …"
            crate::serialization::types::Style::Detailed => {
                if s == "…" {
                    return true;
                }
                s.starts_with('…') && s.ends_with('…')
            }
            // Strict: arrays do not emit omission lines in text mode; treat empty-only as omission
            crate::serialization::types::Style::Strict => s.is_empty(),
        }
    }

    fn rendered_is_pure_text_omission(&self, rendered: &str) -> bool {
        // Consider it pure omission if all non-empty lines match omission pattern for current style.
        let mut any = false;
        for line in rendered.split('\n') {
            if line.trim().is_empty() {
                continue;
            }
            any = true;
            if !Self::is_text_omission_line(self.config.style, line) {
                return false;
            }
        }
        any
    }

    fn source_hint_for(&self, id: usize) -> Option<&'a str> {
        let mut cursor = Some(NodeId(id));
        while let Some(node) = cursor {
            if let Some(key) = self.order.nodes[node.0].key_in_object() {
                return Some(key);
            }
            cursor = self.order.parent.get(node.0).and_then(|parent| *parent);
        }
        self.config.primary_source_name.as_deref()
    }
    fn code_root_array_id(&self, array_id: usize) -> usize {
        let mut current = array_id;
        while let Some(Some(parent)) = self.order.parent.get(current) {
            match self.order.nodes[parent.0] {
                RankedNode::Array { .. } => current = parent.0,
                _ => break,
            }
        }
        current
    }

    fn push_code_line(
        &self,
        child_idx: usize,
        text: &str,
        acc: &mut Vec<Option<String>>,
    ) {
        let idx = self
            .order
            .index_in_parent_array
            .get(child_idx)
            .and_then(|o| *o)
            .unwrap_or(0);
        if acc.len() <= idx {
            acc.resize(idx + 1, None);
        }
        acc[idx] = Some(text.to_string());
    }

    fn collect_code_lines(
        &self,
        array_id: usize,
        acc: &mut Vec<Option<String>>,
    ) {
        if let Some(children) = self.order.children.get(array_id) {
            for child in children {
                let child_idx = child.0;
                match &self.order.nodes[child_idx] {
                    RankedNode::Array { .. } | RankedNode::Object { .. } => {
                        self.collect_code_lines(child_idx, acc);
                    }
                    RankedNode::SplittableLeaf { value, .. } => {
                        self.push_code_line(child_idx, value, acc);
                    }
                    RankedNode::AtomicLeaf { token, .. } => {
                        self.push_code_line(child_idx, token, acc);
                    }
                    RankedNode::LeafPart { .. } => {}
                }
            }
        }
    }

    fn compute_code_highlights(&self, array_id: usize) -> Vec<String> {
        let root = self.code_root_array_id(array_id);
        if let Some(full) = self.order.code_lines.get(&root) {
            let mut highlighter =
                crate::serialization::highlight::CodeHighlighter::new(
                    self.source_hint_for(root),
                );
            return full
                .iter()
                .map(|line| highlighter.highlight_line(line))
                .collect();
        }
        let mut lines: Vec<Option<String>> = Vec::new();
        self.collect_code_lines(array_id, &mut lines);
        if lines.is_empty() {
            return Vec::new();
        }
        let mut highlighter =
            crate::serialization::highlight::CodeHighlighter::new(
                self.source_hint_for(array_id),
            );
        lines
            .into_iter()
            .map(|opt| {
                let text = opt.unwrap_or_default();
                highlighter.highlight_line(&text)
            })
            .collect()
    }

    fn code_highlights_for(
        &mut self,
        array_id: usize,
        template: crate::OutputTemplate,
    ) -> Option<Arc<Vec<String>>> {
        if !matches!(
            self.config.color_strategy(),
            crate::serialization::types::ColorStrategy::Syntax
        ) {
            return None;
        }
        if !matches!(template, crate::OutputTemplate::Code) {
            return None;
        }
        let root = self.code_root_array_id(array_id);
        if let Some(existing) = self.code_highlight_cache.get(&root) {
            return Some(existing.clone());
        }
        let computed = Arc::new(self.compute_code_highlights(root));
        self.code_highlight_cache.insert(root, computed.clone());
        Some(computed)
    }
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
            source_hint: self.source_hint_for(id),
            code_highlight: self.code_highlights_for(id, config.template),
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
        // In non-fileset contexts, Auto uses JSON-family renderer based on style.
        let tmpl = match config.template {
            crate::OutputTemplate::Auto => match config.style {
                crate::serialization::types::Style::Strict => {
                    crate::OutputTemplate::Json
                }
                crate::serialization::types::Style::Default => {
                    crate::OutputTemplate::Pseudo
                }
                crate::serialization::types::Style::Detailed => {
                    crate::OutputTemplate::Js
                }
            },
            other => other,
        };
        render_object(tmpl, &ctx, out)
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Keeps string omission logic in one place for clarity."
    )]
    fn serialize_string(&mut self, id: usize) -> String {
        let kept = self.count_kept_children(id);
        // Number of graphemes to render from the string prefix, honoring any
        // free-prefix allowance enabled in lines-only mode.
        let render_prefix_graphemes =
            match self.config.string_free_prefix_graphemes {
                Some(n) => kept.max(n),
                None => kept,
            };
        let omitted =
            self.omitted_for(id, render_prefix_graphemes).unwrap_or(0);
        let full: &str = match &self.order.nodes[id] {
            RankedNode::SplittableLeaf { value, .. } => value.as_str(),
            _ => unreachable!(
                "serialize_string called for non-string node: id={id}"
            ),
        };
        let truncated_buf = if omitted == 0 {
            None
        } else {
            let prefix = crate::utils::text::take_n_graphemes(
                full,
                render_prefix_graphemes,
            );
            Some(format!("{prefix}…"))
        };
        let raw_for_highlight = truncated_buf.as_deref().unwrap_or(full);
        let highlight_kind = if matches!(
            self.config.template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) {
            HighlightKind::TextLike
        } else {
            HighlightKind::JsonString
        };
        let rendered = if matches!(
            self.config.template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) {
            raw_for_highlight.to_string()
        } else {
            crate::utils::json::json_string(raw_for_highlight)
        };
        self.maybe_highlight_value(
            Some(raw_for_highlight),
            rendered,
            highlight_kind,
        )
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Keeps string omission logic in one place for clarity."
    )]
    fn serialize_string_with_template(
        &mut self,
        id: usize,
        template: crate::serialization::types::OutputTemplate,
    ) -> String {
        let kept = self.count_kept_children(id);
        // Number of graphemes to render from the string prefix, honoring any
        // free-prefix allowance enabled in lines-only mode.
        let render_prefix_graphemes =
            match self.config.string_free_prefix_graphemes {
                Some(n) => kept.max(n),
                None => kept,
            };
        let omitted =
            self.omitted_for(id, render_prefix_graphemes).unwrap_or(0);
        let full: &str = match &self.order.nodes[id] {
            RankedNode::SplittableLeaf { value, .. } => value.as_str(),
            _ => unreachable!(
                "serialize_string called for non-string node: id={id}"
            ),
        };
        let truncated_buf = if omitted == 0 {
            None
        } else {
            let prefix = crate::utils::text::take_n_graphemes(
                full,
                render_prefix_graphemes,
            );
            Some(format!("{prefix}…"))
        };
        let raw_for_highlight = truncated_buf.as_deref().unwrap_or(full);
        let highlight_kind = if matches!(
            template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) {
            HighlightKind::TextLike
        } else {
            HighlightKind::JsonString
        };
        let rendered = if matches!(
            template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) {
            raw_for_highlight.to_string()
        } else {
            crate::utils::json::json_string(raw_for_highlight)
        };
        self.maybe_highlight_value(
            Some(raw_for_highlight),
            rendered,
            highlight_kind,
        )
    }

    fn serialize_atomic(&self, id: usize) -> String {
        let rendered = match &self.order.nodes[id] {
            RankedNode::AtomicLeaf { token, .. } => token.clone(),
            _ => unreachable!("atomic leaf without token: id={id}"),
        };
        self.maybe_highlight_value(None, rendered, HighlightKind::TextLike)
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
                if matches!(
                    self.config.template,
                    crate::serialization::types::OutputTemplate::Text
                        | crate::serialization::types::OutputTemplate::Code
                ) {
                    // For text/code templates, push raw string.
                    out.push_str(&s);
                } else {
                    out.push_string_literal(&s);
                }
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

    #[allow(
        clippy::cognitive_complexity,
        reason = "Text omission filtering adds a branch; clearer inline"
    )]
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
                let child_kind = self.order.nodes[child_id.0].display_kind();
                let rendered =
                    self.render_node_to_string(child_id.0, depth + 1, false);
                // Text template: keep omission-only children so their own
                // renderer can place markers at the correct indentation.
                // Coalesce: if the previously kept child is also a pure
                // omission, skip this one to avoid consecutive markers.
                if matches!(
                    self.config.template,
                    crate::serialization::types::OutputTemplate::Text
                ) && self.rendered_is_pure_text_omission(&rendered)
                {
                    if let Some((_pi, (_pk, prev_s))) = children_pairs.last() {
                        if self.rendered_is_pure_text_omission(prev_s) {
                            continue;
                        }
                    }
                }
                kept += 1;
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

    #[allow(
        clippy::cognitive_complexity,
        reason = "Text omission filtering adds a branch; clearer inline"
    )]
    fn gather_array_children_with_template(
        &mut self,
        id: usize,
        depth: usize,
        template: crate::serialization::types::OutputTemplate,
    ) -> (Vec<ArrayChildPair>, usize) {
        let mut children_pairs: Vec<ArrayChildPair> = Vec::new();
        let mut kept = 0usize;
        if let Some(children_ids) = self.order.children.get(id) {
            for (i, &child_id) in children_ids.iter().enumerate() {
                if self.inclusion_flags[child_id.0] != self.render_set_id {
                    continue;
                }
                let child_kind = self.order.nodes[child_id.0].display_kind();
                let rendered = self.render_node_to_string_with_template(
                    child_id.0,
                    depth + 1,
                    false,
                    template,
                );
                if matches!(
                    template,
                    crate::serialization::types::OutputTemplate::Text
                ) && self.rendered_is_pure_text_omission(&rendered)
                {
                    if let Some((_pi, (_pk, prev_s))) = children_pairs.last() {
                        if self.rendered_is_pure_text_omission(prev_s) {
                            continue;
                        }
                    }
                }
                kept += 1;
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
                let key = self.maybe_highlight_value(
                    Some(raw_key),
                    crate::utils::json::json_string(raw_key),
                    HighlightKind::JsonString,
                );
                let val =
                    self.render_node_to_string(child_id.0, depth + 1, true);
                children_pairs.push((i, (key, val)));
            }
        }
        (children_pairs, kept)
    }

    fn gather_object_children_with_template(
        &mut self,
        id: usize,
        depth: usize,
        template: crate::serialization::types::OutputTemplate,
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
                let key = self.maybe_highlight_value(
                    Some(raw_key),
                    crate::utils::json::json_string(raw_key),
                    HighlightKind::JsonString,
                );
                let val = self.render_node_to_string_with_template(
                    child_id.0,
                    depth + 1,
                    true,
                    template,
                );
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
                let mut ow =
                    Out::new(&mut s, self.config, self.line_number_width);
                self.write_array(id, depth, inline, &mut ow);
                s
            }
            RankedNode::Object { .. } => {
                let mut s = String::new();
                let mut ow =
                    Out::new(&mut s, self.config, self.line_number_width);
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

    // Render helpers that apply a specific OutputTemplate instead of config.template.
    // Enables per-node template overrides (e.g., per-file rendering in filesets).
    fn write_array_with_template(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
        template: crate::serialization::types::OutputTemplate,
    ) {
        let config = self.config;
        let (children_pairs, kept) =
            self.gather_array_children_with_template(id, depth, template);
        let omitted = self.omitted_for(id, kept).unwrap_or(0);
        let ctx = ArrayCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            inline_open: inline,
            omitted_at_start: config.prefer_tail_arrays,
            source_hint: self.source_hint_for(id),
            code_highlight: self.code_highlights_for(id, template),
        };
        render_array(template, &ctx, out)
    }

    fn write_object_with_template(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
        template: crate::serialization::types::OutputTemplate,
    ) {
        let config = self.config;
        let (children_pairs, kept) =
            self.gather_object_children_with_template(id, depth, template);
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
        render_object(template, &ctx, out)
    }

    // Render a node using an explicit OutputTemplate override.
    fn render_node_to_string_with_template(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        template: crate::serialization::types::OutputTemplate,
    ) -> String {
        match &self.order.nodes[id] {
            RankedNode::Array { .. } => {
                let mut s = String::new();
                let mut ow =
                    Out::new(&mut s, self.config, self.line_number_width);
                self.write_array_with_template(
                    id, depth, inline, &mut ow, template,
                );
                s
            }
            RankedNode::Object { .. } => {
                let mut s = String::new();
                let mut ow =
                    Out::new(&mut s, self.config, self.line_number_width);
                self.write_object_with_template(
                    id, depth, inline, &mut ow, template,
                );
                s
            }
            RankedNode::SplittableLeaf { .. } => {
                self.serialize_string_with_template(id, template)
            }
            RankedNode::AtomicLeaf { .. } => self.serialize_atomic(id),
            RankedNode::LeafPart { .. } => {
                unreachable!("string part not rendered")
            }
        }
    }

    fn maybe_highlight_value(
        &self,
        raw: Option<&str>,
        rendered: String,
        kind: HighlightKind,
    ) -> String {
        match self.config.color_strategy() {
            crate::serialization::types::ColorStrategy::None
            | crate::serialization::types::ColorStrategy::Syntax => rendered,
            crate::serialization::types::ColorStrategy::HighlightOnly => {
                if let Some(re) = &self.grep_highlight {
                    return match kind {
                        HighlightKind::JsonString => raw
                            .map(|r| highlight_json_string(re, r))
                            .unwrap_or(rendered),
                        HighlightKind::TextLike => {
                            highlight_matches(re, &rendered)
                        }
                    };
                }
                rendered
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum HighlightKind {
    TextLike,
    JsonString,
}

fn highlight_matches(re: &Regex, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for m in re.find_iter(text) {
        out.push_str(&text[last..m.start()]);
        out.push_str("\u{001b}[31m");
        out.push_str(m.as_str());
        out.push_str("\u{001b}[39m");
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

fn highlight_json_string(re: &Regex, raw: &str) -> String {
    // Build a JSON string literal while inserting highlight escapes around
    // matched spans computed on the raw (unescaped) value.
    let mut out = String::with_capacity(raw.len() + 16);
    out.push('"');
    let mut last = 0usize;
    for m in re.find_iter(raw) {
        out.push_str(&escape_json_fragment(&raw[last..m.start()]));
        out.push_str("\u{001b}[31m");
        out.push_str(&escape_json_fragment(m.as_str()));
        out.push_str("\u{001b}[39m");
        last = m.end();
    }
    out.push_str(&escape_json_fragment(&raw[last..]));
    out.push('"');
    out
}

fn escape_json_fragment(s: &str) -> String {
    let quoted = crate::utils::json::json_string(s);
    // Strip surrounding quotes from a valid JSON string literal.
    quoted[1..quoted.len() - 1].to_string()
}

/// Prepare a render set by including the first `top_k` nodes by priority
/// and all of their ancestors so the output remains structurally valid.
fn enforce_force_first_child(
    order_build: &PriorityOrder,
    inclusion_flags: &mut [u32],
    render_id: u32,
) {
    let priority_index = build_priority_index(order_build);

    for (idx, force) in order_build.force_first_child.iter().enumerate() {
        if !force_child_parent_included(
            inclusion_flags,
            render_id,
            *force,
            idx,
        ) {
            continue;
        }
        let Some(best_child) =
            best_priority_child(order_build, idx, &priority_index)
        else {
            continue;
        };
        if inclusion_flags[best_child.0] == render_id {
            continue;
        }
        crate::utils::graph::mark_node_and_ancestors(
            order_build,
            best_child,
            inclusion_flags,
            render_id,
        );
    }
}

fn build_priority_index(order_build: &PriorityOrder) -> Vec<usize> {
    let mut priority_index = vec![usize::MAX; order_build.total_nodes];
    for (idx, nid) in order_build.by_priority.iter().enumerate() {
        if let Some(slot) = priority_index.get_mut(nid.0) {
            *slot = idx;
        }
    }
    priority_index
}

fn force_child_parent_included(
    inclusion_flags: &[u32],
    render_id: u32,
    force: bool,
    idx: usize,
) -> bool {
    let included =
        inclusion_flags.get(idx).copied().unwrap_or_default() == render_id;
    force && included
}

fn best_priority_child(
    order_build: &PriorityOrder,
    parent_idx: usize,
    priority_index: &[usize],
) -> Option<NodeId> {
    let children = order_build.children.get(parent_idx)?;
    children
        .iter()
        .min_by_key(|cid| {
            priority_index.get(cid.0).copied().unwrap_or(usize::MAX)
        })
        .copied()
}

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
    enforce_force_first_child(order_build, inclusion_flags, render_id);
}

/// Render using a previously prepared render set (inclusion flags matching `render_id`).
pub fn render_from_render_set(
    order_build: &PriorityOrder,
    inclusion_flags: &[u32],
    render_id: u32,
    config: &crate::RenderConfig,
) -> String {
    let root_id = ROOT_PQ_ID;
    // Compute optional global line-number width when numbering is enabled for text.
    fn digits(mut n: usize) -> usize {
        if n == 0 {
            return 1;
        }
        let mut d = 0;
        while n > 0 {
            d += 1;
            n /= 10;
        }
        d
    }
    fn max_index_for_child(
        order: &PriorityOrder,
        flags: &[u32],
        rid: u32,
        child: usize,
    ) -> Option<usize> {
        if flags.get(child).copied().unwrap_or_default() != rid {
            return None;
        }
        match order.nodes[child] {
            RankedNode::AtomicLeaf { .. }
            | RankedNode::SplittableLeaf { .. } => {
                order.index_in_parent_array.get(child).and_then(|idx| *idx)
            }
            RankedNode::Array { .. } | RankedNode::Object { .. } => {
                Some(compute_max_index(order, flags, rid, child))
            }
            _ => None,
        }
    }

    fn compute_max_index(
        order: &PriorityOrder,
        flags: &[u32],
        rid: u32,
        id: usize,
    ) -> usize {
        let mut max_idx = 0usize;
        if let Some(kids) = order.children.get(id) {
            for &cid in kids.iter() {
                let child_id = cid.0;
                if let Some(child_max) =
                    max_index_for_child(order, flags, rid, child_id)
                {
                    if child_max > max_idx {
                        max_idx = child_max;
                    }
                }
            }
        }
        max_idx
    }
    let root_is_fileset =
        order_build.object_type.get(root_id) == Some(&ObjectType::Fileset);
    let should_measure_line_numbers =
        matches!(config.template, crate::OutputTemplate::Code)
            || (matches!(config.template, crate::OutputTemplate::Auto)
                && root_is_fileset);
    let line_number_width = if should_measure_line_numbers {
        let max_index = compute_max_index(
            order_build,
            inclusion_flags,
            render_id,
            root_id,
        );
        Some(digits(max_index.saturating_add(1)))
    } else {
        None
    };
    let mut scope = RenderScope {
        order: order_build,
        inclusion_flags,
        render_set_id: render_id,
        config,
        line_number_width,
        code_highlight_cache: HashMap::new(),
        grep_highlight: config.grep_highlight.clone(),
    };
    let mut s = String::new();
    let mut out = Out::new(&mut s, config, line_number_width);
    scope.write_node(root_id, 0, false, &mut out);
    s
}

/// Convenience: prepare the render set for `top_k` nodes and render in one call.
#[allow(dead_code, reason = "Used by tests and pruner budget measurements")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::types::NodeMetrics;
    use crate::order::{
        NodeId, ObjectType, PriorityOrder, RankedNode, build_order,
    };
    use insta::assert_snapshot;

    fn assert_yaml_valid(s: &str) {
        let _: serde_yaml::Value =
            serde_yaml::from_str(s).expect("YAML parse failed (validation)");
    }

    #[test]
    fn arena_render_empty_array() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Strict,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        assert_snapshot!("arena_render_empty", out);
    }

    #[test]
    fn newline_detection_crlf_array_child() {
        // Ensure we exercise the render_has_newline branch that checks
        // arbitrary newline sequences (e.g., "\r\n") via s.contains(nl).
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Strict,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Strict,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
            line_budget_only: false,
        };
        let arena = crate::ingest::formats::json::build_json_tree_arena(
            "[1,2,3]", &cfg_prio,
        )
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
                style: crate::serialization::types::Style::Default,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
                style: crate::serialization::types::Style::Default,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
            line_budget_only: false,
        };
        let arena = crate::ingest::formats::json::build_json_tree_arena(
            "[1,2,3]", &cfg_prio,
        )
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
                style: crate::serialization::types::Style::Detailed,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
                style: crate::serialization::types::Style::Detailed,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
            line_budget_only: false,
        };
        let arena = crate::ingest::formats::json::build_json_tree_arena(
            "[1,2,3]", &cfg_prio,
        )
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
                style: crate::serialization::types::Style::Detailed,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
                style: crate::serialization::types::Style::Detailed,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        assert_yaml_valid(&out_tail);
        assert_snapshot!("array_omitted_yaml_tail", out_tail);
    }

    #[test]
    fn arena_render_empty_array_yaml() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Default,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        assert_yaml_valid(&out);
        assert_snapshot!("arena_render_empty_yaml", out);
    }

    #[test]
    fn arena_render_single_string_array_yaml() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Default,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        assert_yaml_valid(&out);
        assert_snapshot!("arena_render_single_yaml", out);
    }

    #[test]
    fn inline_open_array_in_object_yaml() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Detailed,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        assert_yaml_valid(&out);
        assert_snapshot!("inline_open_array_in_object_yaml", out);
    }

    #[test]
    fn array_internal_gaps_yaml() {
        let ctx = mk_gap_ctx();
        let mut s = String::new();
        let cfg = test_render_cfg(
            crate::OutputTemplate::Yaml,
            crate::serialization::types::Style::Default,
        );
        let mut outw =
            crate::serialization::output::Out::new(&mut s, &cfg, None);
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
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Default,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Strict,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        // Expect the first 5 characters plus an ellipsis, as a valid JSON string literal.
        assert_eq!(out, "\"abcde…\"");
    }

    #[test]
    fn yaml_array_of_objects_indentation() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Default,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
            style: crate::serialization::types::Style::Strict,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        };
        let scope = RenderScope {
            order: &build,
            inclusion_flags: &marks,
            render_set_id: render_id,
            config: &cfg,
            line_number_width: None,
            code_highlight_cache: HashMap::new(),
            grep_highlight: None,
        };
        // Atomic leaves never report omitted counts.
        let none = scope.omitted_for(crate::order::ROOT_PQ_ID, 0);
        assert!(none.is_none());
    }

    #[test]
    fn inline_open_array_in_object_json() {
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Strict,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
            },
        );
        assert_snapshot!("inline_open_array_in_object_json", out);
    }

    #[test]
    fn arena_render_object_partial_js() {
        // Object with three properties; render top_k small so only one child is kept.
        let arena = crate::ingest::formats::json::build_json_tree_arena(
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
                style: crate::serialization::types::Style::Detailed,
                string_free_prefix_graphemes: None,
                debug: false,
                primary_source_name: None,
                show_fileset_headers: true,
                fileset_tree: false,
                count_fileset_headers_in_budgets: false,
                grep_highlight: None,
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

    fn mk_gap_ctx() -> super::templates::ArrayCtx<'static> {
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
            source_hint: None,
            code_highlight: None,
        }
    }

    fn assert_contains_all(out: &str, needles: &[&str]) {
        needles.iter().for_each(|n| assert!(out.contains(n)));
    }

    fn test_render_cfg(
        template: crate::OutputTemplate,
        style: crate::serialization::types::Style,
    ) -> crate::RenderConfig {
        crate::RenderConfig {
            template,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: crate::ColorMode::Off,
            color_enabled: false,
            style,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: false,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        }
    }

    #[test]
    fn array_internal_gaps_pseudo() {
        let ctx = mk_gap_ctx();
        let mut s = String::new();
        let cfg = test_render_cfg(
            crate::OutputTemplate::Pseudo,
            crate::serialization::types::Style::Default,
        );
        let mut outw =
            crate::serialization::output::Out::new(&mut s, &cfg, None);
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
        let cfg = test_render_cfg(
            crate::OutputTemplate::Js,
            crate::serialization::types::Style::Default,
        );
        let mut outw =
            crate::serialization::output::Out::new(&mut s, &cfg, None);
        super::templates::render_array(
            crate::OutputTemplate::Js,
            &ctx,
            &mut outw,
        );
        let out = s;
        assert!(out.contains("/* 2 more items */"));
        assert!(out.contains("/* 1 more items */"));
    }

    #[test]
    fn force_child_prefers_highest_priority_child() {
        // Parent has two children; child with PQ id 2 has higher global priority
        // than child with PQ id 1, but force-first-child currently pulls the
        // first listed child. This captures the undesired behavior.
        let order = PriorityOrder {
            metrics: vec![NodeMetrics::default(); 3],
            nodes: vec![
                RankedNode::Array {
                    node_id: NodeId(0),
                    key_in_object: None,
                },
                RankedNode::Array {
                    node_id: NodeId(1),
                    key_in_object: None,
                },
                RankedNode::Array {
                    node_id: NodeId(2),
                    key_in_object: None,
                },
            ],
            scores: vec![0, 0, 0],
            parent: vec![None, Some(NodeId(0)), Some(NodeId(0))],
            children: vec![
                vec![NodeId(1), NodeId(2)], // first child = NodeId(1)
                Vec::new(),
                Vec::new(),
            ],
            index_in_parent_array: vec![None, Some(0), Some(1)],
            by_priority: vec![NodeId(0), NodeId(2), NodeId(1)], // child 2 outranks child 1
            total_nodes: 3,
            object_type: vec![ObjectType::Object; 3],
            force_first_child: vec![true, false, false],
            code_lines: HashMap::new(),
            fileset_children: None,
        };
        let mut flags = Vec::new();
        let render_id = 1u32;
        prepare_render_set_top_k_and_ancestors(
            &order, 1, &mut flags, render_id,
        );
        assert_eq!(
            flags.get(2).copied().unwrap_or_default(),
            render_id,
            "expected highest-priority child to be pulled in alongside force-first parent"
        );
        assert_ne!(
            flags.get(1).copied().unwrap_or_default(),
            render_id,
            "lower-priority first child should not be forced when a higher-priority sibling exists"
        );
    }
}
