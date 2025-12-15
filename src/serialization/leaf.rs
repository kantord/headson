use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::order::PriorityOrder;
use crate::order::RankedNode;

use super::highlight::{HighlightKind, maybe_highlight_value};

pub(super) struct LeafRenderer<'a> {
    order: &'a PriorityOrder,
    config: &'a crate::RenderConfig,
    grep_highlight: Option<Regex>,
    code_highlight_cache: HashMap<usize, Arc<Vec<String>>>,
    source_hint: Box<dyn Fn(usize) -> Option<&'a str> + 'a>,
}

impl<'a> LeafRenderer<'a> {
    pub(super) fn new<F>(
        order: &'a PriorityOrder,
        config: &'a crate::RenderConfig,
        grep_highlight: Option<Regex>,
        source_hint: F,
    ) -> Self
    where
        F: Fn(usize) -> Option<&'a str> + 'a,
    {
        Self {
            order,
            config,
            grep_highlight,
            code_highlight_cache: HashMap::new(),
            source_hint: Box::new(source_hint),
        }
    }

    pub(super) fn grep_highlight(&self) -> &Option<Regex> {
        &self.grep_highlight
    }

    pub(super) fn source_hint(&self, id: usize) -> Option<&'a str> {
        (self.source_hint)(id)
    }

    pub(super) fn omitted_for_string(
        &self,
        id: usize,
        kept: usize,
    ) -> Option<usize> {
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

    pub(super) fn omitted_for(&self, id: usize, kept: usize) -> Option<usize> {
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

    pub(super) fn serialize_string_for_template(
        &mut self,
        id: usize,
        kept_graphemes: usize,
        template: crate::serialization::types::OutputTemplate,
    ) -> String {
        let render_prefix = self.render_prefix_len(kept_graphemes);
        let omitted = self.omitted_for(id, render_prefix).unwrap_or(0);
        let full = self.full_string(id);
        let truncated_buf =
            self.truncated_display(full, render_prefix, omitted);
        let raw_for_highlight = truncated_buf.as_deref().unwrap_or(full);
        let rendered = self.render_for_template(raw_for_highlight, template);
        let highlight_kind = self.highlight_kind_for(template);
        maybe_highlight_value(
            self.config,
            Some(raw_for_highlight),
            rendered,
            highlight_kind,
            &self.grep_highlight,
        )
    }

    pub(super) fn serialize_atomic(&self, id: usize) -> String {
        let rendered = match &self.order.nodes[id] {
            RankedNode::AtomicLeaf { token, .. } => token.clone(),
            _ => unreachable!("atomic leaf without token: id={id}"),
        };
        maybe_highlight_value(
            self.config,
            None,
            rendered,
            HighlightKind::TextLike,
            &self.grep_highlight,
        )
    }

    pub(super) fn code_highlights_for(
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
        let root = crate::serialization::highlight::code_root_array_id(
            self.order, array_id,
        );
        if let Some(existing) = self.code_highlight_cache.get(&root) {
            return Some(existing.clone());
        }
        let computed =
            Arc::new(crate::serialization::highlight::code_highlight_lines(
                self.order,
                root,
                (self.source_hint)(root),
            ));
        self.code_highlight_cache.insert(root, computed.clone());
        Some(computed)
    }

    fn render_prefix_len(&self, kept_graphemes: usize) -> usize {
        match self.config.string_free_prefix_graphemes {
            Some(n) => kept_graphemes.max(n),
            None => kept_graphemes,
        }
    }

    fn full_string(&self, id: usize) -> &str {
        match &self.order.nodes[id] {
            RankedNode::SplittableLeaf { value, .. } => value.as_str(),
            _ => unreachable!(
                "serialize_string_for_template called for non-string node: id={id}"
            ),
        }
    }

    fn truncated_display(
        &self,
        full: &str,
        render_prefix: usize,
        omitted: usize,
    ) -> Option<String> {
        if omitted == 0 {
            None
        } else {
            let prefix =
                crate::utils::text::take_n_graphemes(full, render_prefix);
            Some(format!("{prefix}…"))
        }
    }

    fn highlight_kind_for(
        &self,
        template: crate::serialization::types::OutputTemplate,
    ) -> HighlightKind {
        if matches!(
            template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) {
            HighlightKind::TextLike
        } else {
            HighlightKind::JsonString
        }
    }

    fn render_for_template(
        &self,
        raw_for_highlight: &str,
        template: crate::serialization::types::OutputTemplate,
    ) -> String {
        if matches!(
            template,
            crate::serialization::types::OutputTemplate::Text
                | crate::serialization::types::OutputTemplate::Code
        ) {
            raw_for_highlight.to_string()
        } else {
            crate::utils::json::json_string(raw_for_highlight)
        }
    }
}
