use super::color;

// Simple output layer that centralizes colored and structured pushes
// while still rendering into a String buffer (to preserve sizing/measurement).
pub struct Out<'a> {
    buf: &'a mut String,
    newline: String,
    indent_unit: String,
    color_enabled: bool,
}

impl<'a> Out<'a> {
    pub fn new(
        buf: &'a mut String,
        newline: &str,
        indent_unit: &str,
        color_enabled: bool,
    ) -> Self {
        Self {
            buf,
            newline: newline.to_string(),
            indent_unit: indent_unit.to_string(),
            color_enabled,
        }
    }

    pub fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    pub fn push_char(&mut self, c: char) {
        self.buf.push(c);
    }

    pub fn push_newline(&mut self) {
        self.buf.push_str(&self.newline);
    }

    pub fn push_indent(&mut self, depth: usize) {
        self.buf.push_str(&self.indent_unit.repeat(depth));
    }

    pub fn push_comment<S: Into<String>>(&mut self, body: S) {
        let s = color::color_comment(body, self.color_enabled);
        self.buf.push_str(&s);
    }

    pub fn push_omission(&mut self) {
        self.buf
            .push_str(color::omission_marker(self.color_enabled));
    }

    // Color role helpers for tokens
    pub fn push_key(&mut self, quoted_key: &str) {
        let s = color::wrap_role(
            quoted_key,
            color::ColorRole::Key,
            self.color_enabled,
        );
        self.buf.push_str(&s);
    }

    pub fn push_string_literal(&mut self, quoted_value: &str) {
        let s = color::wrap_role(
            quoted_value,
            color::ColorRole::String,
            self.color_enabled,
        );
        self.buf.push_str(&s);
    }

    pub fn push_number_literal(&mut self, number_text: &str) {
        let s = color::wrap_role(
            number_text,
            color::ColorRole::Number,
            self.color_enabled,
        );
        self.buf.push_str(&s);
    }

    pub fn push_bool(&mut self, val: bool) {
        let s = color::wrap_role(
            if val { "true" } else { "false" },
            color::ColorRole::Bool,
            self.color_enabled,
        );
        self.buf.push_str(&s);
    }

    pub fn push_null(&mut self) {
        let s = color::wrap_role(
            "null",
            color::ColorRole::Null,
            self.color_enabled,
        );
        self.buf.push_str(&s);
    }

    // Formatting mode queries
    pub fn is_compact_mode(&self) -> bool {
        self.newline.is_empty() && self.indent_unit.is_empty()
    }
}
