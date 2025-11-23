// File-format specific ingest adapters live under this module.
pub mod json;
pub mod text;
pub mod yaml;

// Re-export commonly used helpers for convenience
pub use json::parse_json_one;
pub use text::{parse_text_many, parse_text_one, parse_text_one_with_mode};
pub use yaml::parse_yaml_one;
