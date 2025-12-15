use crate::order::ObjectType;
use crate::order::{NodeId, NodeKind, PriorityOrder, ROOT_PQ_ID, RankedNode};

use super::leaf::LeafRenderer;
use super::output::Out;
use super::templates::{ArrayCtx, ObjectCtx, render_array, render_object};

type ArrayChildPair = (usize, (NodeKind, String));
type ObjectChildPair = (usize, (String, String));

pub(crate) struct RenderEngine<'a> {
    pub(crate) order: &'a PriorityOrder,
    pub(crate) inclusion_flags: &'a [u32],
    pub(crate) render_set_id: u32,
    pub(crate) config: &'a crate::RenderConfig,
    pub(crate) line_number_width: Option<usize>,
    pub(crate) slot_map: Option<&'a [Option<usize>]>,
    pub(crate) leaf: LeafRenderer<'a>,
}

impl<'a> RenderEngine<'a> {
    pub(crate) fn new(
        order: &'a PriorityOrder,
        inclusion_flags: &'a [u32],
        render_set_id: u32,
        config: &'a crate::RenderConfig,
        line_number_width: Option<usize>,
        slot_map: Option<&'a [Option<usize>]>,
    ) -> Self {
        let source_hint = {
            let order_ref = order;
            move |id: usize| -> Option<&'a str> {
                let mut cursor = Some(NodeId(id));
                while let Some(node) = cursor {
                    if let Some(key) = order_ref.nodes[node.0].key_in_object()
                    {
                        return Some(key);
                    }
                    cursor = order_ref
                        .parent
                        .get(node.0)
                        .and_then(|parent| *parent);
                }
                config.primary_source_name.as_deref()
            }
        };
        let grep_highlight = config.grep_highlight.clone();
        let leaf =
            LeafRenderer::new(order, config, grep_highlight, source_hint);
        Self {
            order,
            inclusion_flags,
            render_set_id,
            config,
            line_number_width,
            slot_map,
            leaf,
        }
    }

    fn slot_for(&self, node_id: usize) -> Option<usize> {
        self.slot_map
            .and_then(|slots| slots.get(node_id).copied().flatten())
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

    pub(crate) fn write_array(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
    ) {
        let config = self.config;
        let (children_pairs, kept) = self.gather_array_children_with_template(
            id,
            depth,
            config.template,
        );
        let omitted = self.leaf.omitted_for(id, kept).unwrap_or(0);
        let ctx = ArrayCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            inline_open: inline,
            omitted_at_start: config.prefer_tail_arrays,
            source_hint: self.leaf.source_hint(id),
            code_highlight: self.leaf.code_highlights_for(id, config.template),
        };
        render_array(config.template, &ctx, out)
    }

    pub(crate) fn write_object(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
    ) {
        let config = self.config;
        if self.try_render_fileset_root(id, depth, out) {
            return;
        }
        let (children_pairs, kept) = self
            .gather_object_children_with_template(id, depth, config.template);
        let omitted = self.leaf.omitted_for(id, kept).unwrap_or(0);
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

    pub(crate) fn write_node(
        &mut self,
        id: usize,
        depth: usize,
        inline: bool,
        out: &mut Out<'_>,
    ) {
        out.set_current_slot(self.slot_for(id));
        match &self.order.nodes[id] {
            RankedNode::Array { .. } => {
                self.write_array(id, depth, inline, out)
            }
            RankedNode::Object { .. } => {
                self.write_object(id, depth, inline, out)
            }
            RankedNode::SplittableLeaf { .. } => {
                let kept = self.count_kept_children(id);
                let s = self.leaf.serialize_string_for_template(
                    id,
                    kept,
                    self.config.template,
                );
                if matches!(
                    self.config.template,
                    crate::serialization::types::OutputTemplate::Text
                        | crate::serialization::types::OutputTemplate::Code
                ) {
                    out.push_str(&s);
                } else {
                    out.push_string_literal(&s);
                }
            }
            RankedNode::AtomicLeaf { .. } => {
                let s = self.leaf.serialize_atomic(id);
                out.push_str(&s);
            }
            RankedNode::LeafPart { .. } => {
                unreachable!("string part should not be rendered")
            }
        }
    }

    fn gather_array_children_with_template(
        &mut self,
        id: usize,
        depth: usize,
        template: crate::serialization::types::OutputTemplate,
    ) -> (Vec<ArrayChildPair>, usize) {
        let Some(children_ids) = self.order.children.get(id) else {
            return (Vec::new(), 0);
        };
        let mut kept = 0usize;
        let mut pairs: Vec<ArrayChildPair> = Vec::new();
        for (i, &child_id) in children_ids.iter().enumerate() {
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                continue;
            }
            kept += 1;
            let child_kind = self.order.nodes[child_id.0].display_kind();
            let rendered = self.render_node_to_string_with_template(
                child_id.0,
                depth + 1,
                false,
                template,
            );
            let orig_index = self
                .order
                .index_in_parent_array
                .get(child_id.0)
                .copied()
                .flatten()
                .unwrap_or(i);
            pairs.push((orig_index, (child_kind, rendered)));
        }
        (pairs, kept)
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
                let key = super::highlight::maybe_highlight_value(
                    self.config,
                    Some(raw_key),
                    crate::utils::json::json_string(raw_key),
                    super::highlight::HighlightKind::JsonString,
                    self.leaf.grep_highlight(),
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

    pub(crate) fn write_array_with_template(
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
        let omitted = self.leaf.omitted_for(id, kept).unwrap_or(0);
        let ctx = ArrayCtx {
            children: children_pairs,
            children_len: kept,
            omitted,
            depth,
            inline_open: inline,
            omitted_at_start: config.prefer_tail_arrays,
            source_hint: self.leaf.source_hint(id),
            code_highlight: self.leaf.code_highlights_for(id, template),
        };
        render_array(template, &ctx, out)
    }

    pub(crate) fn write_object_with_template(
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
        let omitted = self.leaf.omitted_for(id, kept).unwrap_or(0);
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

    pub(crate) fn render_node_to_string_with_template(
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
                let kept = self.count_kept_children(id);
                self.leaf.serialize_string_for_template(id, kept, template)
            }
            RankedNode::AtomicLeaf { .. } => self.leaf.serialize_atomic(id),
            RankedNode::LeafPart { .. } => {
                unreachable!("string part not rendered")
            }
        }
    }
}
