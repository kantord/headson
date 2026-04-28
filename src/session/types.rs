use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Breadcrumb {
    pub file: String,
    pub path: String,
    pub count: u64,
    pub last_step: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryEntry {
    pub step: u64,
    pub timestamp: String,
    pub cwd: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: String,
    pub label: String,
    pub step_count: u64,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub queries: Vec<QueryEntry>,
}

impl Session {
    pub fn new(id: String, label: String) -> Self {
        Self {
            id,
            label,
            step_count: 0,
            breadcrumbs: vec![],
            queries: vec![],
        }
    }

    pub fn new_with_cwd(id: String, cwd: &str) -> Self {
        let label = format!("Explore session started originally in {cwd}");
        Self::new(id, label)
    }

    pub fn record_breadcrumb(&mut self, file: &str, path: &str, step: u64) {
        if let Some(entry) = self
            .breadcrumbs
            .iter_mut()
            .find(|b| b.file == file && b.path == path)
        {
            entry.count += 1;
            entry.last_step = step;
        } else {
            self.breadcrumbs.push(Breadcrumb {
                file: file.to_string(),
                path: path.to_string(),
                count: 1,
                last_step: step,
            });
        }
    }

    pub fn penalty_for(
        &self,
        file: &str,
        path: &str,
        current_step: u64,
        alpha: f64,
    ) -> f64 {
        match self
            .breadcrumbs
            .iter()
            .find(|b| b.file == file && b.path == path)
        {
            None => 0.0,
            Some(b) => {
                (1.0 + b.count as f64).ln()
                    * alpha.powi((current_step - b.last_step) as i32)
            }
        }
    }

    pub fn evict(&mut self, current_step: u64, alpha: f64, cap: usize) {
        // Epsilon prune: drop entries whose decay factor is below 0.001
        self.breadcrumbs.retain(|b| {
            alpha.powi((current_step - b.last_step) as i32) >= 0.001
        });
        // Cap: keep only the `cap` most recently seen entries
        if self.breadcrumbs.len() > cap {
            self.breadcrumbs
                .sort_by(|a, b| b.last_step.cmp(&a.last_step));
            self.breadcrumbs.truncate(cap);
        }
    }

    pub fn clear(&mut self) {
        self.breadcrumbs = vec![];
        self.step_count = 0;
    }

    pub fn record_query(
        &mut self,
        step: u64,
        timestamp: &str,
        cwd: &str,
        argv: Vec<String>,
    ) {
        self.step_count += 1;
        self.queries.push(QueryEntry {
            step,
            timestamp: timestamp.to_string(),
            cwd: cwd.to_string(),
            argv,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_count_increments_on_record_query() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        assert_eq!(session.step_count, 0);
        session.record_query(1, "t1", "/", vec![]);
        session.record_query(2, "t2", "/", vec![]);
        assert_eq!(session.step_count, 2);
    }

    #[test]
    fn record_query_stores_correct_fields() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_query(
            1,
            "2026-01-01T00:00:00Z",
            "/home/user",
            vec!["src/".into(), "-C".into(), "8000".into()],
        );
        assert_eq!(session.queries.len(), 1);
        let entry = &session.queries[0];
        assert_eq!(entry.step, 1);
        assert_eq!(entry.timestamp, "2026-01-01T00:00:00Z");
        assert_eq!(entry.cwd, "/home/user");
        assert_eq!(entry.argv, vec!["src/", "-C", "8000"]);
    }

    #[test]
    fn clear_zeroes_breadcrumbs_and_step_count_preserves_rest() {
        let mut session = Session::new("x".to_string(), "lbl".to_string());
        session.step_count = 5;
        session.record_breadcrumb("a.json", "x", 1);
        session.record_breadcrumb("b.json", "y", 2);
        session.record_breadcrumb("c.json", "z", 3);
        session.queries.push(QueryEntry {
            step: 1,
            timestamp: "t1".to_string(),
            cwd: "/".to_string(),
            argv: vec![],
        });
        session.queries.push(QueryEntry {
            step: 2,
            timestamp: "t2".to_string(),
            cwd: "/".to_string(),
            argv: vec![],
        });
        session.clear();
        assert!(session.breadcrumbs.is_empty());
        assert_eq!(session.step_count, 0);
        assert_eq!(session.label, "lbl");
        assert_eq!(session.id, "x");
        assert_eq!(session.queries.len(), 2);
    }

    #[test]
    fn cap_eviction_truncates_to_limit_retaining_most_recent() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        // Add 5 breadcrumbs all above epsilon threshold at current_step=6, alpha=0.5
        // last_steps: 5, 4, 3, 2, 1 — all recent enough
        for (i, last_step) in [5u64, 4, 3, 2, 1].iter().enumerate() {
            session.record_breadcrumb(
                "file.json",
                &format!("key.{i}"),
                *last_step,
            );
            // Override last_step to the desired value (record_breadcrumb sets it to 1 always on first insert)
            session.breadcrumbs.last_mut().unwrap().last_step = *last_step;
        }
        session.evict(6, 0.5, 3);
        assert_eq!(session.breadcrumbs.len(), 3);
        // The 3 retained should have the highest last_step values: 5, 4, 3
        let mut retained_steps: Vec<u64> =
            session.breadcrumbs.iter().map(|b| b.last_step).collect();
        retained_steps.sort();
        assert_eq!(retained_steps, vec![3, 4, 5]);
    }

    #[test]
    fn epsilon_prune_drops_fully_decayed_entries() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb("file.json", "old.path", 0);
        session.evict(50, 0.5, 10000);
        assert!(session.breadcrumbs.is_empty());
    }

    #[test]
    fn more_recently_seen_node_has_higher_penalty() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb("file.json", "key_a", 4);
        session.record_breadcrumb("file.json", "key_b", 1);
        let p_a = session.penalty_for("file.json", "key_a", 5, 0.5);
        let p_b = session.penalty_for("file.json", "key_b", 5, 0.5);
        assert!(p_a > p_b);
    }

    #[test]
    fn penalty_formula_produces_correct_value() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb("file.json", "a.b", 3);
        let p = session.penalty_for("file.json", "a.b", 5, 0.5);
        let expected = f64::ln(2.0) * 0.5_f64.powi(2);
        assert!((p - expected).abs() < 1e-12);
    }

    #[test]
    fn penalty_for_unseen_node_is_zero() {
        let session = Session::new("id".to_string(), "lbl".to_string());
        let p = session.penalty_for("file.json", "missing.path", 5, 0.5);
        assert_eq!(p, 0.0);
    }

    #[test]
    fn record_breadcrumb_increments_existing_entry() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb("file.json", "a.b", 1);
        session.record_breadcrumb("file.json", "a.b", 2);
        assert_eq!(session.breadcrumbs.len(), 1);
        assert_eq!(session.breadcrumbs[0].count, 2);
        assert_eq!(session.breadcrumbs[0].last_step, 2);
    }

    #[test]
    fn record_breadcrumb_creates_new_entry() {
        let mut session = Session::new("id".to_string(), "lbl".to_string());
        session.record_breadcrumb("file.json", "users.0.name", 1);
        assert_eq!(session.breadcrumbs.len(), 1);
        assert_eq!(session.breadcrumbs[0].count, 1);
        assert_eq!(session.breadcrumbs[0].last_step, 1);
    }

    #[test]
    fn new_with_cwd_auto_labels_from_path() {
        let session =
            Session::new_with_cwd("id1".to_string(), "/home/user/project");
        assert_eq!(
            session.label,
            "Explore session started originally in /home/user/project"
        );
    }

    #[test]
    fn session_round_trips_through_serde() {
        let session = Session::new("s1".to_string(), "lbl".to_string());
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "s1");
        assert_eq!(deserialized.label, "lbl");
        assert_eq!(deserialized.step_count, 0);
        assert!(deserialized.breadcrumbs.is_empty());
        assert!(deserialized.queries.is_empty());
    }
}
