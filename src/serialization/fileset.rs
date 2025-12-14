use super::engine::RenderEngine;
use crate::ingest::format::Format;
use crate::order::{NodeId, ObjectType, ROOT_PQ_ID};
use crate::serialization::color::{self, ColorRole};
use crate::serialization::types::OutputTemplate;

impl<'a> RenderEngine<'a> {
    pub(super) fn try_render_fileset_root(
        &mut self,
        id: usize,
        depth: usize,
        out: &mut crate::serialization::output::Out<'_>,
    ) -> bool {
        if id == ROOT_PQ_ID
            && self.order.object_type.get(id) == Some(&ObjectType::Fileset)
            && !self.config.newline.is_empty()
        {
            if self.config.fileset_tree {
                self.render_fileset_tree(depth, out);
                return true;
            }
            self.render_fileset_sections(depth, out);
            return true;
        }
        false
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Tree assembly mixes omission tracking and rendering prep; further splitting would obscure the flow."
    )]
    fn render_fileset_tree(
        &mut self,
        depth: usize,
        out: &mut crate::serialization::output::Out<'_>,
    ) {
        let Some(children_ids) =
            self.fileset_children().map(<[NodeId]>::to_vec)
        else {
            return;
        };
        let inputs = self.collect_tree_inputs(&children_ids, depth);
        if inputs.is_empty() {
            return;
        }

        let mut root = TreeNode::root();
        self.build_tree(&mut root, &inputs);

        let show_scaffold = self.config.show_fileset_headers;
        self.render_tree_output(root, depth, show_scaffold, out);
    }

    fn render_fileset_sections(
        &mut self,
        depth: usize,
        out: &mut crate::serialization::output::Out<'_>,
    ) {
        let Some(children_ids) = self
            .order
            .fileset_children
            .as_deref()
            .or_else(|| self.order.children.get(ROOT_PQ_ID).map(|v| &**v))
        else {
            return;
        };
        let show_headers = self.should_render_fileset_headers();
        let kept = self.render_fileset_children(
            children_ids,
            depth,
            show_headers,
            out,
        );
        if show_headers {
            self.render_fileset_summary(children_ids, depth, kept, out);
        }
    }

    fn fileset_push_section_gap(
        &self,
        out: &mut crate::serialization::output::Out<'_>,
    ) {
        let nl = &self.config.newline;
        out.push_str(nl);
        out.push_str(nl);
    }

    fn should_render_fileset_headers(&self) -> bool {
        self.config.show_fileset_headers
            && !self.config.newline.is_empty()
            && !self.config.fileset_tree
    }

    fn render_fileset_children(
        &mut self,
        children_ids: &[NodeId],
        depth: usize,
        show_headers: bool,
        out: &mut crate::serialization::output::Out<'_>,
    ) -> usize {
        let mut kept = 0usize;
        for (slot_idx, &child_id) in children_ids.iter().enumerate() {
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                continue;
            }
            if kept > 0 && show_headers {
                out.set_current_slot(None);
                self.fileset_push_section_gap(out);
            }
            kept += 1;
            let raw_key =
                self.order.nodes[child_id.0].key_in_object().unwrap_or("");
            if show_headers {
                out.set_current_slot(Some(slot_idx));
                out.push_str(&self.fileset_header_line(depth, raw_key));
            }
            out.set_current_slot(Some(slot_idx));
            let rendered =
                self.fileset_render_child(child_id.0, depth, raw_key);
            out.push_str(&rendered);
        }
        kept
    }

    fn fileset_children(&self) -> Option<&[NodeId]> {
        self.order
            .fileset_children
            .as_deref()
            .or_else(|| self.order.children.get(ROOT_PQ_ID).map(|v| &**v))
    }

    fn collect_tree_inputs(
        &mut self,
        children_ids: &[NodeId],
        depth: usize,
    ) -> TreeInputs {
        let mut inputs = TreeInputs::default();
        for (slot_idx, &child_id) in children_ids.iter().enumerate() {
            let raw_key =
                self.order.nodes[child_id.0].key_in_object().unwrap_or("");
            let segments = Self::split_path_segments(raw_key);
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                inputs.track_omission_for_path(&segments);
                continue;
            }
            let rendered =
                self.fileset_render_child(child_id.0, depth, raw_key);
            inputs.entries.push((segments, rendered, slot_idx));
        }
        inputs
    }

    fn build_tree(&self, root: &mut TreeNode, inputs: &TreeInputs) {
        for (segments, rendered, slot) in &inputs.entries {
            root.insert(*slot, segments, rendered.clone(), self.config);
        }
        let mut omitted = inputs.omitted_map.clone();
        omitted.insert(Vec::<String>::new(), inputs.root_direct_omitted);
        for path in inputs
            .omitted_paths_in_order
            .iter()
            .filter(|k| !k.is_empty())
        {
            root.ensure_path(path);
        }
        root.apply_omitted_counts(&omitted, &mut Vec::new());
    }

    fn render_tree_output(
        &self,
        root: TreeNode,
        depth: usize,
        render_scaffold_lines: bool,
        out: &mut crate::serialization::output::Out<'_>,
    ) {
        let indent = self.config.indent_unit.repeat(depth);
        if render_scaffold_lines {
            out.set_current_slot(None);
            out.push_str(&indent);
            out.push_char('.');
            out.push_str(&self.config.newline);
        }
        let mut root_children = root.children;
        if root.omitted > 0 {
            root_children.push(TreeNode::omission(root.omitted));
        }
        let last_idx = root_children.len().saturating_sub(1);
        for (idx, child) in root_children.into_iter().enumerate() {
            child.render(
                out,
                &indent,
                idx == last_idx,
                self.config,
                render_scaffold_lines,
            );
        }
    }

    fn render_fileset_summary(
        &self,
        children_ids: &[NodeId],
        depth: usize,
        kept: usize,
        out: &mut crate::serialization::output::Out<'_>,
    ) {
        let total = self
            .order
            .metrics
            .get(ROOT_PQ_ID)
            .and_then(|m| m.object_len)
            .unwrap_or(children_ids.len());
        if total > kept && !self.config.newline.is_empty() {
            out.set_current_slot(None);
            self.fileset_push_section_gap(out);
            out.set_current_slot(None);
            out.push_str(&self.fileset_summary_line(depth, total - kept));
        }
    }

    fn fileset_header_line(&self, depth: usize, key: &str) -> String {
        let nl = &self.config.newline;
        let indent = self.config.indent_unit.repeat(depth);
        let mut s = String::with_capacity(indent.len() + key.len() + 8);
        s.push_str(&indent);
        s.push_str("==> ");
        s.push_str(key);
        s.push_str(" <==");
        s.push_str(nl);
        s
    }

    fn fileset_summary_line(&self, depth: usize, omitted: usize) -> String {
        let indent = self.config.indent_unit.repeat(depth);
        format!("{indent}==> {omitted} more files <==")
    }

    fn fileset_render_child(
        &mut self,
        child_id: usize,
        depth: usize,
        raw_key: &str,
    ) -> String {
        if self.config.count_fileset_headers_in_budgets
            && !self.node_has_included_descendants(child_id)
            && !self.node_is_included_leaf(child_id)
            && self.node_has_children(child_id)
        {
            // When headers consume the entire per-file budget, skip rendering
            // a body/omission marker so we don't exceed the caller’s cap.
            return String::new();
        }
        if matches!(self.config.template, OutputTemplate::Auto) {
            let template = self.fileset_template_for(raw_key);
            return self.render_node_to_string_with_template(
                child_id, depth, false, template,
            );
        }
        self.render_node_to_string_with_template(
            child_id,
            depth,
            false,
            self.config.template,
        )
    }

    fn fileset_template_for(&self, raw_key: &str) -> OutputTemplate {
        match Format::from_filename(raw_key) {
            Format::Yaml => OutputTemplate::Yaml,
            Format::Json => match self.config.style {
                crate::serialization::types::Style::Strict => {
                    OutputTemplate::Json
                }
                crate::serialization::types::Style::Default => {
                    OutputTemplate::Pseudo
                }
                crate::serialization::types::Style::Detailed => {
                    OutputTemplate::Js
                }
            },
            Format::Unknown => {
                if crate::utils::extensions::is_code_like_name(raw_key) {
                    OutputTemplate::Code
                } else {
                    OutputTemplate::Text
                }
            }
        }
    }

    fn node_has_included_descendants(&self, node_idx: usize) -> bool {
        let mut stack: Vec<usize> = self
            .order
            .children
            .get(node_idx)
            .map(|kids| kids.iter().map(|k| k.0).collect())
            .unwrap_or_default();
        while let Some(idx) = stack.pop() {
            if self.inclusion_flags.get(idx).copied()
                == Some(self.render_set_id)
            {
                return true;
            }
            if let Some(kids) = self.order.children.get(idx) {
                stack.extend(kids.iter().map(|k| k.0));
            }
        }
        false
    }

    fn node_has_children(&self, node_idx: usize) -> bool {
        self.order
            .children
            .get(node_idx)
            .map(|c| !c.is_empty())
            .unwrap_or(false)
    }

    fn node_is_included_leaf(&self, node_idx: usize) -> bool {
        self.inclusion_flags
            .get(node_idx)
            .copied()
            .is_some_and(|flag| flag == self.render_set_id)
            && matches!(
                self.order.nodes.get(node_idx),
                Some(
                    crate::RankedNode::AtomicLeaf { .. }
                        | crate::RankedNode::SplittableLeaf { .. }
                        | crate::RankedNode::LeafPart { .. }
                )
            )
    }

    fn split_path_segments(raw_key: &str) -> Vec<String> {
        let segments: Vec<String> = raw_key
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        if segments.is_empty() {
            vec![raw_key.to_string()]
        } else {
            segments
        }
    }
}

#[derive(Default)]
struct TreeInputs {
    entries: Vec<(Vec<String>, String, usize)>,
    omitted_map: std::collections::HashMap<Vec<String>, usize>,
    omitted_paths_in_order: Vec<Vec<String>>,
    root_direct_omitted: usize,
}

impl TreeInputs {
    fn track_omission_for_path(&mut self, segments: &[String]) {
        if segments.len() > 1 {
            let mut prefix: Vec<String> = Vec::new();
            for seg in &segments[..segments.len() - 1] {
                prefix.push(seg.clone());
                let entry =
                    self.omitted_map.entry(prefix.clone()).or_insert(0);
                if *entry == 0 {
                    self.omitted_paths_in_order.push(prefix.clone());
                }
                *entry += 1;
            }
        } else {
            // Root-level omission (no folder to pin it to).
            self.root_direct_omitted += 1;
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self.omitted_map.is_empty()
            && self.root_direct_omitted == 0
    }
}

struct TreeNode {
    name: String,
    slot: Option<usize>,
    children: Vec<TreeNode>,
    content: Option<Vec<String>>,
    omitted: usize,
    is_omission: bool,
}

struct CollapsedNode {
    name: String,
    slot: Option<usize>,
    children: Vec<TreeNode>,
    content: Option<Vec<String>>,
    omitted: usize,
    is_omission: bool,
}

impl TreeNode {
    fn root() -> Self {
        TreeNode {
            name: ".".to_string(),
            slot: None,
            children: Vec::new(),
            content: None,
            omitted: 0,
            is_omission: false,
        }
    }

    fn with_name(name: String) -> Self {
        TreeNode {
            name,
            slot: None,
            children: Vec::new(),
            content: None,
            omitted: 0,
            is_omission: false,
        }
    }

    fn insert(
        &mut self,
        slot: usize,
        segments: &[String],
        rendered: String,
        config: &crate::RenderConfig,
    ) {
        if segments.is_empty() {
            return;
        }
        let head = &segments[0];
        if segments.len() == 1 {
            let mut node = Self::with_name(head.clone());
            node.slot = Some(slot);
            node.content = Some(Self::render_lines(rendered, config));
            self.children.push(node);
            return;
        }
        let mut child_idx = None;
        for (idx, child) in self.children.iter().enumerate() {
            if child.name == *head {
                child_idx = Some(idx);
                break;
            }
        }
        let idx = if let Some(idx) = child_idx {
            idx
        } else {
            self.children.push(Self::with_name(head.clone()));
            self.children.len() - 1
        };
        self.children[idx].insert(slot, &segments[1..], rendered, config);
    }

    fn ensure_path(&mut self, segments: &[String]) {
        if segments.is_empty() {
            return;
        }
        let head = &segments[0];
        let mut idx = None;
        for (i, child) in self.children.iter().enumerate() {
            if child.name == *head {
                idx = Some(i);
                break;
            }
        }
        let idx = idx.unwrap_or_else(|| {
            self.children.push(Self::with_name(head.clone()));
            self.children.len() - 1
        });
        if segments.len() > 1 {
            self.children[idx].ensure_path(&segments[1..]);
        }
    }

    fn apply_omitted_counts(
        &mut self,
        counts: &std::collections::HashMap<Vec<String>, usize>,
        path: &mut Vec<String>,
    ) {
        self.omitted = counts.get(path).copied().unwrap_or(0);
        for child in &mut self.children {
            path.push(child.name.clone());
            child.apply_omitted_counts(counts, path);
            path.pop();
        }
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Tree render branches are simple; splitting further would hurt clarity"
    )]
    fn render(
        self,
        out: &mut crate::serialization::output::Out<'_>,
        prefix: &str,
        is_last: bool,
        config: &crate::RenderConfig,
        render_scaffold_lines: bool,
    ) {
        let collapsed = self.collapse();
        let mut children = collapsed.children;
        let is_leaf = collapsed.content.is_some();
        let content = collapsed.content;
        let omitted = collapsed.omitted;
        let slot = collapsed.slot;
        let slot_for_scaffold = if render_scaffold_lines
            && !config.count_fileset_headers_in_budgets
        {
            None
        } else {
            slot
        };
        let nl = &config.newline;
        // Tree scaffolding (pipes/names) keeps syntax coloring even in
        // highlight-only grep mode. Those glyphs never receive grep highlights,
        // so this avoids double-highlighting concerns while preserving legibility.
        let color_on = config.color_enabled;
        if render_scaffold_lines {
            let branch = match (is_leaf, is_last) {
                (false, true) => "└─ ",
                (true, _) | (false, false) => "├─ ",
            };
            out.set_current_slot(slot_for_scaffold);
            out.push_str(prefix);
            out.push_str(&colorize_pipe(branch, color_on));
            let display_name = if is_leaf {
                collapsed.name
            } else {
                format!("{}/", collapsed.name)
            };
            if collapsed.is_omission {
                out.push_str(&colorize_pipe(&display_name, color_on));
            } else {
                out.push_str(&colorize_name(&display_name, color_on));
            }
            out.push_str(nl);
        } else if collapsed.is_omission {
            out.set_current_slot(slot);
            out.push_str(&collapsed.name);
            out.push_str(nl);
        }

        let child_prefix = if render_scaffold_lines {
            format!("{prefix}{} ", colorize_pipe("│", color_on))
        } else {
            String::new()
        };
        if let Some(lines) = content {
            for line in lines {
                let prefix_slot = if render_scaffold_lines
                    && !config.count_fileset_headers_in_budgets
                {
                    slot_for_scaffold
                } else {
                    slot
                };
                out.set_current_slot(prefix_slot);
                out.push_str(&child_prefix);
                out.set_current_slot(slot);
                out.push_str(&line);
                out.push_str(nl);
            }
        }
        if omitted > 0 {
            children.push(TreeNode::omission(omitted));
        }
        let last_idx = children.len().saturating_sub(1);
        for (idx, child) in children.into_iter().enumerate() {
            child.render(
                out,
                &child_prefix,
                idx == last_idx,
                config,
                render_scaffold_lines,
            );
        }
    }

    fn render_lines(
        rendered: String,
        config: &crate::RenderConfig,
    ) -> Vec<String> {
        if config.newline.is_empty() {
            return vec![rendered];
        }
        let mut lines: Vec<String> = rendered
            .split(&config.newline)
            .map(ToString::to_string)
            .collect();
        if matches!(lines.last(), Some(s) if s.is_empty()) {
            lines.pop();
        }
        lines
    }

    fn collapse(self) -> CollapsedNode {
        let mut name = self.name;
        let mut slot = self.slot;
        let mut content = self.content;
        let mut children = self.children;
        let mut omitted = self.omitted;
        let mut is_omission = self.is_omission;
        while content.is_none()
            && omitted == 0
            && children.len() == 1
            && children[0].omitted == 0
        {
            if let Some(child) = children.pop() {
                name = format!("{name}/{}", child.name);
                slot = slot.or(child.slot);
                content = child.content;
                omitted = omitted.saturating_add(child.omitted);
                children = child.children;
                is_omission = child.is_omission;
            } else {
                break;
            }
        }
        CollapsedNode {
            name,
            slot,
            children,
            content,
            omitted,
            is_omission,
        }
    }

    fn omission(count: usize) -> Self {
        TreeNode {
            name: format!("… {count} more items"),
            slot: None,
            children: Vec::new(),
            content: Some(Vec::new()),
            omitted: 0,
            is_omission: true,
        }
    }
}

fn colorize_pipe(s: &str, enabled: bool) -> String {
    color::color_comment(s, enabled)
}

fn colorize_name(s: &str, enabled: bool) -> String {
    color::wrap_role(s, ColorRole::Key, enabled)
}

#[cfg(test)]
mod tests {
    use super::TreeNode;
    use crate::{
        ColorMode, OutputTemplate, RenderConfig, serialization::types::Style,
    };
    use std::collections::HashMap;

    fn render_tree_from_node(
        root: TreeNode,
        config: &RenderConfig,
        render_scaffold_lines: bool,
    ) -> String {
        let mut buf = String::new();
        let mut out =
            crate::serialization::output::Out::new(&mut buf, config, None);
        if render_scaffold_lines {
            out.set_current_slot(None);
            out.push_char('.');
            out.push_str(&config.newline);
        }
        let mut root_children = root.children;
        if root.omitted > 0 {
            root_children.push(TreeNode::omission(root.omitted));
        }
        let last_idx = root_children.len().saturating_sub(1);
        for (idx, child) in root_children.into_iter().enumerate() {
            child.render(
                &mut out,
                "",
                idx == last_idx,
                config,
                render_scaffold_lines,
            );
        }
        buf
    }

    #[test]
    fn tree_scaffolding_is_free_when_disabled() {
        // Measurement pass disables scaffold lines; content should render without tree pipes.
        let config = RenderConfig {
            template: OutputTemplate::Auto,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: true,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        };

        let mut root = TreeNode::root();
        root.insert(
            0,
            &["a.txt".to_string()],
            "line one\nline two\n".to_string(),
            &config,
        );

        let out = render_tree_from_node(root, &config, false);
        let expected = concat!("line one\n", "line two\n");
        assert_eq!(
            out, expected,
            "scaffolding should not be included when render_scaffold_lines is false"
        );
    }

    #[test]
    fn collapse_carries_omitted_counts() {
        let mut root = TreeNode::root();
        root.children.push(TreeNode::with_name("a".to_string()));
        let mut counts = std::collections::HashMap::new();
        counts.insert(Vec::<String>::new(), 1);
        counts.insert(vec!["a".to_string()], 1);
        root.apply_omitted_counts(&counts, &mut Vec::new());

        let collapsed = root.collapse();
        assert_eq!(collapsed.name, ".");
        assert!(collapsed.content.is_none());
        assert_eq!(collapsed.omitted, 1, "root should track omitted items");
        assert_eq!(
            collapsed.children.first().map(|c| c.omitted),
            Some(1),
            "child path should still carry its omission count"
        );
    }

    #[test]
    fn tree_reports_omitted_files_once() {
        // Regression test: a single omitted file should not be surfaced twice
        // (previously showed both under its directory and again at the fileset root).
        let config = RenderConfig {
            template: OutputTemplate::Auto,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: true,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        };

        let mut root = TreeNode::root();
        // Include one file under dir/.
        root.insert(
            0,
            &["dir".to_string(), "kept.txt".to_string()],
            "line\n".to_string(),
            &config,
        );
        // Simulate omitting a sibling file under the same directory.
        let mut counts = HashMap::new();
        counts.insert(Vec::<String>::new(), 0); // root omission count
        counts.insert(vec!["dir".to_string()], 1); // dir/ omission count
        root.apply_omitted_counts(&counts, &mut Vec::new());

        let out = render_tree_from_node(root, &config, true);

        // Desired behavior: omission should be reported once under the containing folder.
        let expected = concat!(
            ".\n",
            "└─ dir/\n",
            "│ ├─ kept.txt\n",
            "│ │ line\n",
            "│ ├─ … 1 more items\n",
        );
        assert_eq!(
            out, expected,
            "a single omitted file currently renders two omission markers (root + dir)"
        );
    }

    #[test]
    fn tree_scopes_omission_to_nested_folder() {
        let config = RenderConfig {
            template: OutputTemplate::Auto,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: true,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        };

        let mut root = TreeNode::root();
        root.insert(
            0,
            &[
                "dir".to_string(),
                "nested".to_string(),
                "keep.rs".to_string(),
            ],
            "fn keep() {}\n".to_string(),
            &config,
        );
        let mut counts = HashMap::new();
        counts.insert(Vec::<String>::new(), 0);
        counts.insert(vec!["dir".to_string()], 1);
        counts.insert(vec!["dir".to_string(), "nested".to_string()], 1);
        root.apply_omitted_counts(&counts, &mut Vec::new());

        let out = render_tree_from_node(root, &config, true);
        let expected = concat!(
            ".\n",
            "└─ dir/\n",
            "│ ├─ nested/\n",
            "│ │ ├─ keep.rs\n",
            "│ │ │ fn keep() {}\n",
            "│ │ ├─ … 1 more items\n",
            "│ ├─ … 1 more items\n",
        );
        assert_eq!(
            out, expected,
            "nested omissions should be reported under their folder without duplicating at root"
        );
    }

    #[test]
    fn tree_root_level_omission_when_no_children_kept() {
        let config = RenderConfig {
            template: OutputTemplate::Auto,
            indent_unit: "  ".to_string(),
            space: " ".to_string(),
            newline: "\n".to_string(),
            prefer_tail_arrays: false,
            color_mode: ColorMode::Off,
            color_enabled: false,
            style: Style::Default,
            string_free_prefix_graphemes: None,
            debug: false,
            primary_source_name: None,
            show_fileset_headers: true,
            fileset_tree: true,
            count_fileset_headers_in_budgets: false,
            grep_highlight: None,
        };

        let mut root = TreeNode::root();
        let mut counts = HashMap::new();
        counts.insert(Vec::<String>::new(), 2);
        root.apply_omitted_counts(&counts, &mut Vec::new());

        let out = render_tree_from_node(root, &config, true);
        let expected = concat!(".\n", "├─ … 2 more items\n",);
        assert_eq!(
            out, expected,
            "when no files are kept, omissions should only appear once at the root"
        );
    }
}
