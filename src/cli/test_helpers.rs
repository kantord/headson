/// Shared test isolation helper for tests that mutate XDG_STATE_HOME and/or
/// HSON_SESSION environment variables. Restores prior values on drop.
pub(crate) struct IsolatedEnv {
    old_state: Option<String>,
    old_session: Option<String>,
}

impl IsolatedEnv {
    pub(crate) fn new(
        state_dir: &std::path::Path,
        session_id: Option<&str>,
    ) -> Self {
        let old_state = std::env::var("XDG_STATE_HOME").ok();
        let old_session = std::env::var("HSON_SESSION").ok();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", state_dir);
            match session_id {
                Some(id) => std::env::set_var("HSON_SESSION", id),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }
        Self {
            old_state,
            old_session,
        }
    }
}

impl Drop for IsolatedEnv {
    fn drop(&mut self) {
        unsafe {
            match &self.old_state {
                Some(v) => std::env::set_var("XDG_STATE_HOME", v),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
            match &self.old_session {
                Some(v) => std::env::set_var("HSON_SESSION", v),
                None => std::env::remove_var("HSON_SESSION"),
            }
        }
    }
}
