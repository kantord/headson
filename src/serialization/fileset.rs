use super::RenderScope;

impl<'a> RenderScope<'a> {
    // Keep fileset layout policy isolated from item templates.
    pub(super) fn try_render_fileset_root(
        &mut self,
        _id: usize,
        _depth: usize,
    ) -> Option<String> {
        None
    }
}
