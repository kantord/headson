mod common;

fn run_with_paths(paths: &[&str], budget: usize) -> (String, String) {
    let budget_s = budget.to_string();
    // Auto format selects per-file JSON renderings for .json inputs.
    let mut args =
        vec!["--no-color", "--no-sort", "-c", &budget_s, "-f", "auto"];
    args.extend_from_slice(paths);
    let out = common::run_cli(&args, None);
    (out.stdout, out.stderr)
}

#[test]
fn multiple_input_paths_render_in_sections() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    // Use a large budget to include both entries fully
    let (out, _err) = run_with_paths(&[p1, p2], 100_000);
    assert!(out.contains("==> "));
    assert!(out.contains(p1));
    assert!(out.contains(p2));
}

#[test]
fn json_template_sections_headers_present_for_multi_file() {
    let p1 = "tests/fixtures/explicit/object_small.json";
    let p2 = "tests/fixtures/explicit/array_numbers_50.json";
    let (out, _err) = run_with_paths(&[p1, p2], 100_000);
    assert!(out.contains("==> "));
    assert!(out.contains(p1));
    assert!(out.contains(p2));
}
