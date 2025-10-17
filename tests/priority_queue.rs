use insta::assert_snapshot;
use serde_json::Value;

#[test]
fn pq_empty_array() {
    let value: Value = serde_json::from_str("[]").unwrap();
    let pq = headson::build_priority_queue(&value).unwrap();
    let mut lines = vec![format!("len={}", pq.len())];
    for (item, prio) in pq.into_sorted_iter() {
        lines.push(format!("{:?} prio={}", item, prio));
    }
    assert_snapshot!("pq_empty_array_queue", lines.join("\n"));
}

#[test]
fn pq_single_string_array() {
    let value: Value = serde_json::from_str("[\"ab\"]").unwrap();
    let pq = headson::build_priority_queue(&value).unwrap();
    let mut lines = vec![format!("len={}", pq.len())];
    for (item, prio) in pq.into_sorted_iter() {
        lines.push(format!("{:?} prio={}", item, prio));
    }
    assert_snapshot!("pq_single_string_array_queue", lines.join("\n"));
}
