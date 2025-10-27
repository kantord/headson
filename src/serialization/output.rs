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

    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    #[inline]
    pub fn push_char(&mut self, c: char) {
        self.buf.push(c);
    }

    #[inline]
    pub fn push_newline(&mut self) {
        self.buf.push_str(&self.newline);
    }

    #[inline]
    pub fn push_indent(&mut self, depth: usize) {
        self.buf.push_str(&self.indent_unit.repeat(depth));
    }

    #[inline]
    pub fn push_comment<S: Into<String>>(&mut self, body: S) {
        let s = color::color_comment(body, self.color_enabled);
        self.buf.push_str(&s);
    }

    #[inline]
    pub fn push_omission(&mut self) {
        self.buf
            .push_str(color::omission_marker(self.color_enabled));
    }
}
