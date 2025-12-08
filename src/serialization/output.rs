use super::color;
use crate::utils::measure::OutputStats;
use crate::utils::measure::{count_line_breaks, ends_with_break};

#[derive(Clone, Debug)]
pub struct SlotStatsRecorder {
    per_slot: Vec<SlotRunning>,
    measure_chars: bool,
}

#[derive(Clone, Debug)]
struct SlotRunning {
    bytes: usize,
    chars: usize,
    breaks: usize,
    ends_with_break: bool,
    has_content: bool,
}

impl SlotRunning {
    fn new() -> Self {
        Self {
            bytes: 0,
            chars: 0,
            breaks: 0,
            ends_with_break: false,
            has_content: false,
        }
    }
}

impl SlotStatsRecorder {
    pub fn new(slot_count: usize, measure_chars: bool) -> Self {
        Self {
            per_slot: vec![SlotRunning::new(); slot_count],
            measure_chars,
        }
    }

    pub fn add_chunk(&mut self, slot: Option<usize>, chunk: &str) {
        let Some(idx) = slot else { return };
        if chunk.is_empty() {
            return;
        }
        if let Some(running) = self.per_slot.get_mut(idx) {
            running.bytes = running.bytes.saturating_add(chunk.len());
            if self.measure_chars {
                running.chars =
                    running.chars.saturating_add(chunk.chars().count());
            }
            let b = chunk.as_bytes();
            running.breaks =
                running.breaks.saturating_add(count_line_breaks(b));
            running.ends_with_break = ends_with_break(b);
            running.has_content = true;
        }
    }

    pub fn into_output_stats(self) -> Vec<OutputStats> {
        self.per_slot
            .into_iter()
            .map(|r| {
                if !r.has_content {
                    return OutputStats {
                        bytes: 0,
                        chars: 0,
                        lines: 0,
                    };
                }
                let mut lines = r.breaks.saturating_add(1);
                if r.ends_with_break && lines > 0 {
                    lines -= 1;
                }
                OutputStats {
                    bytes: r.bytes,
                    chars: if self.measure_chars { r.chars } else { 0 },
                    lines,
                }
            })
            .collect()
    }
}

// Simple output layer that centralizes colored and structured pushes
// while still rendering into a String buffer (to preserve sizing/measurement).
pub struct Out<'a> {
    buf: &'a mut String,
    newline: String,
    indent_unit: String,
    // Syntax/role colors are only emitted when both color_enabled is true
    // and the strategy allows syntax coloring (ColorStrategy::Syntax).
    role_colors_enabled: bool,
    style: crate::serialization::types::Style,
    line_number_width: Option<usize>,
    recorder: Option<SlotStatsRecorder>,
    current_slot: Option<usize>,
}

impl<'a> Out<'a> {
    pub fn new(
        buf: &'a mut String,
        config: &crate::RenderConfig,
        line_number_width: Option<usize>,
    ) -> Self {
        Self::new_with_recorder(buf, config, line_number_width, None)
    }

    pub fn new_with_recorder(
        buf: &'a mut String,
        config: &crate::RenderConfig,
        line_number_width: Option<usize>,
        recorder: Option<SlotStatsRecorder>,
    ) -> Self {
        let role_colors_enabled = matches!(
            config.color_strategy(),
            crate::serialization::types::ColorStrategy::Syntax
        );
        Self {
            buf,
            newline: config.newline.clone(),
            indent_unit: config.indent_unit.clone(),
            role_colors_enabled,
            style: config.style,
            line_number_width,
            recorder,
            current_slot: None,
        }
    }

    pub fn set_current_slot(&mut self, slot: Option<usize>) {
        self.current_slot = slot;
    }

    fn record_chunk(&mut self, s: &str) {
        if let Some(rec) = self.recorder.as_mut() {
            rec.add_chunk(self.current_slot, s);
        }
    }

    pub fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
        self.record_chunk(s);
    }

    pub fn push_char(&mut self, c: char) {
        self.buf.push(c);
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.record_chunk(s);
    }

    pub fn push_newline(&mut self) {
        let nl = self.newline.clone();
        self.buf.push_str(&nl);
        self.record_chunk(&nl);
    }

    pub fn push_indent(&mut self, depth: usize) {
        let s = self.indent_unit.repeat(depth);
        self.record_chunk(&s);
        self.buf.push_str(&s);
    }

    pub fn push_comment<S: Into<String>>(&mut self, body: S) {
        let s = color::color_comment(body, self.role_colors_enabled);
        self.buf.push_str(&s);
        self.record_chunk(&s);
    }

    pub fn push_omission(&mut self) {
        let s = color::omission_marker(self.role_colors_enabled);
        self.record_chunk(s);
        self.buf.push_str(s);
    }

    // Color role helpers for tokens
    pub fn push_key(&mut self, quoted_key: &str) {
        let s = color::wrap_role(
            quoted_key,
            color::ColorRole::Key,
            self.role_colors_enabled,
        );
        self.buf.push_str(&s);
        self.record_chunk(&s);
    }

    pub fn push_string_literal(&mut self, quoted_value: &str) {
        let s = color::wrap_role(
            quoted_value,
            color::ColorRole::String,
            self.role_colors_enabled,
        );
        self.buf.push_str(&s);
        self.record_chunk(&s);
    }

    // Push an unquoted string value using the string color role.
    pub fn push_string_unquoted(&mut self, value: &str) {
        let s = color::wrap_role(
            value,
            color::ColorRole::String,
            self.role_colors_enabled,
        );
        self.buf.push_str(&s);
        self.record_chunk(&s);
    }

    pub fn into_slot_stats(self) -> Option<Vec<OutputStats>> {
        self.recorder.map(SlotStatsRecorder::into_output_stats)
    }

    // Formatting mode queries
    pub fn is_compact_mode(&self) -> bool {
        self.newline.is_empty() && self.indent_unit.is_empty()
    }

    pub fn style(&self) -> crate::serialization::types::Style {
        self.style
    }

    pub fn line_number_width(&self) -> Option<usize> {
        self.line_number_width
    }

    pub fn colors_enabled(&self) -> bool {
        self.role_colors_enabled
    }
}
