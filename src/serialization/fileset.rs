use super::RenderScope;
use crate::ingest::format::Format;
use crate::order::{NodeId, ObjectType, ROOT_PQ_ID};
use crate::serialization::color::{self, ColorRole};
use crate::serialization::types::ColorStrategy;
use crate::serialization::types::OutputTemplate;

impl<'a> RenderScope<'a> {
    pub(super) fn try_render_fileset_root(
        &mut self,
        id: usize,
        depth: usize,
    ) -> Option<String> {
        if id == ROOT_PQ_ID
            && self.order.object_type.get(id) == Some(&ObjectType::Fileset)
            && !self.config.newline.is_empty()
        {
            if self.config.fileset_tree {
                return Some(self.render_fileset_tree(depth));
            }
            return Some(self.render_fileset_sections(depth));
        }
        None
    }

    fn render_fileset_tree(&mut self, depth: usize) -> String {
        let Some(children_ids) = self
            .order
            .fileset_children
            .as_deref()
            .or_else(|| self.order.children.get(ROOT_PQ_ID).map(|v| &**v))
        else {
            return String::new();
        };
        let mut entries: Vec<(Vec<String>, String)> = Vec::new();
        for &child_id in children_ids {
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                continue;
            }
            let raw_key =
                self.order.nodes[child_id.0].key_in_object().unwrap_or("");
            let rendered =
                self.fileset_render_child(child_id.0, depth, raw_key);
            let segments = Self::split_path_segments(raw_key);
            entries.push((segments, rendered));
        }
        if entries.is_empty() {
            return String::new();
        }

        let mut root = TreeNode::root();
        for (segments, rendered) in entries {
            root.insert(&segments, rendered, self.config);
        }

        let mut out = String::new();
        let indent = self.config.indent_unit.repeat(depth);
        out.push_str(&indent);
        out.push('.');
        out.push_str(&self.config.newline);
        let last_idx = root.children.len().saturating_sub(1);
        for (idx, child) in root.children.into_iter().enumerate() {
            child.render(&mut out, &indent, idx == last_idx, self.config);
        }
        out
    }

    fn render_fileset_sections(&mut self, depth: usize) -> String {
        let Some(children_ids) = self
            .order
            .fileset_children
            .as_deref()
            .or_else(|| self.order.children.get(ROOT_PQ_ID).map(|v| &**v))
        else {
            return String::new();
        };
        let show_headers = self.should_render_fileset_headers();
        let mut out = String::new();
        let kept = self.render_fileset_children(
            children_ids,
            depth,
            show_headers,
            &mut out,
        );
        if show_headers {
            self.render_fileset_summary(children_ids, depth, kept, &mut out);
        }
        out
    }

    fn fileset_push_section_gap(&self, out: &mut String) {
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
        out: &mut String,
    ) -> usize {
        let mut kept = 0usize;
        for &child_id in children_ids {
            if self.inclusion_flags[child_id.0] != self.render_set_id {
                continue;
            }
            if kept > 0 && show_headers {
                self.fileset_push_section_gap(out);
            }
            kept += 1;
            let raw_key =
                self.order.nodes[child_id.0].key_in_object().unwrap_or("");
            if show_headers {
                out.push_str(&self.fileset_header_line(depth, raw_key));
            }
            let rendered =
                self.fileset_render_child(child_id.0, depth, raw_key);
            out.push_str(&rendered);
        }
        kept
    }

    fn render_fileset_summary(
        &self,
        children_ids: &[NodeId],
        depth: usize,
        kept: usize,
        out: &mut String,
    ) {
        let total = self
            .order
            .metrics
            .get(ROOT_PQ_ID)
            .and_then(|m| m.object_len)
            .unwrap_or(children_ids.len());
        if total > kept && !self.config.newline.is_empty() {
            self.fileset_push_section_gap(out);
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
        if matches!(self.config.template, OutputTemplate::Auto) {
            let template = self.fileset_template_for(raw_key);
            return self.render_node_to_string_with_template(
                child_id, depth, false, template,
            );
        }
        self.render_node_to_string(child_id, depth, false)
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

struct TreeNode {
    name: String,
    children: Vec<TreeNode>,
    content: Option<Vec<String>>,
}

impl TreeNode {
    fn root() -> Self {
        TreeNode {
            name: ".".to_string(),
            children: Vec::new(),
            content: None,
        }
    }

    fn with_name(name: String) -> Self {
        TreeNode {
            name,
            children: Vec::new(),
            content: None,
        }
    }

    fn insert(
        &mut self,
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
        self.children[idx].insert(&segments[1..], rendered, config);
    }

    #[allow(
        clippy::cognitive_complexity,
        reason = "Tree render branches are simple; splitting further would hurt clarity"
    )]
    fn render(
        self,
        out: &mut String,
        prefix: &str,
        is_last: bool,
        config: &crate::RenderConfig,
    ) {
        let (name, children, content) = self.collapse();
        let nl = &config.newline;
        let is_leaf = content.is_some();
        let branch = match (is_leaf, is_last) {
            (false, true) => "└─ ",
            (true, _) | (false, false) => "├─ ",
        };
        let color_on = !matches!(config.color_strategy(), ColorStrategy::None);
        out.push_str(prefix);
        out.push_str(&colorize_pipe(branch, color_on));
        let display_name = if content.is_none() {
            format!("{name}/")
        } else {
            name
        };
        out.push_str(&colorize_name(&display_name, color_on));
        out.push_str(nl);

        let child_prefix =
            format!("{prefix}{} ", colorize_pipe("│", color_on));
        if let Some(lines) = content {
            for line in lines {
                out.push_str(&child_prefix);
                out.push_str(&line);
                out.push_str(nl);
            }
        }
        let last_idx = children.len().saturating_sub(1);
        for (idx, child) in children.into_iter().enumerate() {
            child.render(out, &child_prefix, idx == last_idx, config);
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

    fn collapse(self) -> (String, Vec<TreeNode>, Option<Vec<String>>) {
        let mut name = self.name;
        let mut content = self.content;
        let mut children = self.children;
        while content.is_none() && children.len() == 1 {
            if let Some(child) = children.pop() {
                name = format!("{name}/{}", child.name);
                content = child.content;
                children = child.children;
            } else {
                break;
            }
        }
        (name, children, content)
    }
}

fn colorize_pipe(s: &str, enabled: bool) -> String {
    color::color_comment(s, enabled)
}

fn colorize_name(s: &str, enabled: bool) -> String {
    color::wrap_role(s, ColorRole::Key, enabled)
}
