/// Logical data formats detected from filenames or paths.
/// This is independent from input ingestion; it controls how an item
/// should be rendered within a fileset based on its filename extension.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Format {
    Json,
    Yaml,
    Unknown,
}

impl Format {
    /// Map a filename or path string to a `Format` by inspecting its extension.
    /// Uses `Path::extension` and ASCII case-insensitive comparison to avoid
    /// allocations. Known mappings:
    /// - .json -> Json
    /// - .yaml, .yml -> Yaml
    pub fn from_filename(name: &str) -> Self {
        use std::path::Path;
        const EXT_FORMATS: &[(&str, Format)] = &[
            ("json", Format::Json),
            ("yaml", Format::Yaml),
            ("yml", Format::Yaml),
        ];
        if let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str())
        {
            for (pat, fmt) in EXT_FORMATS {
                if ext.eq_ignore_ascii_case(pat) {
                    return *fmt;
                }
            }
        }
        Format::Unknown
    }

    /// Preferred output template for this format. Falls back to caller’s
    /// template for `Unknown`.
    #[allow(dead_code, reason = "kept for compatibility and potential reuse")]
    pub fn to_output_template(
        self,
        fallback: crate::serialization::types::OutputTemplate,
    ) -> crate::serialization::types::OutputTemplate {
        match self {
            Format::Json => crate::serialization::types::OutputTemplate::Json,
            Format::Yaml => crate::serialization::types::OutputTemplate::Yaml,
            Format::Unknown => fallback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Format;

    #[test]
    #[allow(
        clippy::cognitive_complexity,
        reason = "Single test covers multiple assertions compactly."
    )]
    fn maps_common_extensions() {
        assert_eq!(Format::from_filename("a.json"), Format::Json);
        assert_eq!(Format::from_filename("b.yaml"), Format::Yaml);
        assert_eq!(Format::from_filename("c.yml"), Format::Yaml);
        assert_eq!(Format::from_filename("d.JSON"), Format::Json);
        assert_eq!(Format::from_filename("e.YmL"), Format::Yaml);
        assert_eq!(Format::from_filename("noext"), Format::Unknown);
        assert_eq!(Format::from_filename("weird.tar.gz"), Format::Unknown);
    }
}
