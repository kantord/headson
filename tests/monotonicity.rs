#[path = "../test_support/mod.rs"]
mod util;
use std::fs;

fn trimmed_len(s: &str) -> usize {
    s.trim_end_matches(['\r', '\n']).len()
}

fn collect_lengths(
    path: &str,
    template: &str,
    budgets: &[usize],
) -> Vec<usize> {
    let input = fs::read_to_string(path).expect("read fixture");
    budgets
        .iter()
        .map(|&b| {
            trimmed_len(&util::run_template_budget(&input, template, b, &[]))
        })
        .collect()
}

fn assert_monotonic(lens: &[usize], budgets: &[usize]) {
    for i in 1..lens.len() {
        assert!(
            lens[i] >= lens[i - 1],
            "non-decreasing: {} >= {} (b{} -> b{})",
            lens[i],
            lens[i - 1],
            budgets[i - 1],
            budgets[i]
        );
    }
}

fn assert_within_budget_or_min(
    lens: &[usize],
    budgets: &[usize],
    path: &str,
    template: &str,
) {
    let min_nonzero =
        lens.iter().copied().filter(|n| *n > 0).min().unwrap_or(0);
    for (i, &b) in budgets.iter().enumerate() {
        if b == 0 {
            assert_budget_zero(lens[i], template, path);
        } else if min_nonzero > 0 && b < min_nonzero {
            if lens[i] == 0 {
                // Under stricter budget enforcement we may render nothing when no
                // content fits; accept empty output as valid.
                continue;
            }
            assert_min_nonzero(lens[i], min_nonzero, b, template, path);
        } else {
            assert_within_budget(lens[i], b, template, path);
        }
    }
}

fn assert_budget_zero(len: usize, template: &str, path: &str) {
    assert_eq!(
        len, 0,
        "budget=0 should suppress output (template={template}, path={path})"
    );
}

fn assert_min_nonzero(
    len: usize,
    min_nonzero: usize,
    budget: usize,
    template: &str,
    path: &str,
) {
    assert_eq!(
        len, min_nonzero,
        "should use minimal preview when budget < min_nonzero (b={budget}, template={template}, path={path})"
    );
}

fn assert_within_budget(
    len: usize,
    budget: usize,
    template: &str,
    path: &str,
) {
    assert!(
        len <= budget,
        "len={len} should be <= budget={budget} (template={template}, path={path})",
    );
}

#[test]
fn object_small_monotonic_and_within_budget() {
    let budgets = [0usize, 1, 5, 10, 20, 50, 100, 1000];
    for &tmpl in &["json", "pseudo", "js"] {
        let path = "tests/fixtures/explicit/object_small.json";
        let lens = collect_lengths(path, tmpl, &budgets);
        assert_monotonic(&lens, &budgets);
        assert_within_budget_or_min(&lens, &budgets, path, tmpl);
    }
}

#[test]
fn array_numbers_50_monotonic_and_within_budget() {
    let budgets = [0usize, 1, 5, 10, 20, 30, 60, 120];
    for &tmpl in &["json", "pseudo", "js"] {
        let path = "tests/fixtures/explicit/array_numbers_50.json";
        let lens = collect_lengths(path, tmpl, &budgets);
        assert_monotonic(&lens, &budgets);
        assert_within_budget_or_min(&lens, &budgets, path, tmpl);
    }
}
