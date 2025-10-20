#[cfg(test)]
mod tests {
    use crate::serialization::render_arena_with_marks;
    use insta::assert_snapshot;

    #[test]
    fn arena_render_empty_array() {
        let arena = crate::stream_arena::build_stream_arena(
            "[]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = crate::build_priority_order_from_arena(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_arena_with_marks(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
            },
        )
        .unwrap();
        assert_snapshot!("arena_render_empty", out);
    }

    #[test]
    fn arena_render_single_string_array() {
        let arena = crate::stream_arena::build_stream_arena(
            "[\"ab\"]",
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let build = crate::build_priority_order_from_arena(
            &arena,
            &crate::PriorityConfig::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let mut marks = vec![0u32; build.total_nodes];
        let out = render_arena_with_marks(
            &build,
            10,
            &mut marks,
            1,
            &crate::RenderConfig {
                template: crate::OutputTemplate::Json,
                indent_unit: "  ".to_string(),
                space: " ".to_string(),
                newline: "\n".to_string(),
            },
        )
        .unwrap();
        assert_snapshot!("arena_render_single", out);
    }
}
