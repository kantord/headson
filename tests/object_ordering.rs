#[path = "../test_support/mod.rs"]
mod util;

fn run(input: &str, template: &str, budget: usize) -> String {
    util::run_template_budget(input, template, budget, &[])
}

#[test]
fn object_key_order_is_stable_full() {
    let a = r#"{"b":1,"a":2,"c":0}"#;
    let b = r#"{"c":0,"b":1,"a":2}"#;
    for &tmpl in &["json", "pseudo", "js"] {
        let out_a = run(a, tmpl, 1000);
        let out_b = run(b, tmpl, 1000);
        assert_eq!(out_a, out_b, "template={tmpl}");
        // Basic ordering check: "a" before "b" before "c"
        let s = out_a;
        let pa = s.find("\"a\"").unwrap();
        let pb = s.find("\"b\"").unwrap();
        let pc = s.find("\"c\"").unwrap();
        assert!(
            pa < pb && pb < pc,
            "keys should be lexicographic a<b<c: {s:?}"
        );
    }
}

#[test]
fn object_key_order_is_stable_under_truncation() {
    let a = r#"{"b":1,"a":2,"c":0,"d":3}"#;
    let b = r#"{"d":3,"c":0,"b":1,"a":2}"#;
    for &tmpl in &["json", "pseudo", "js"] {
        for &budget in &[10usize, 30usize, 60usize] {
            let out_a = run(a, tmpl, budget);
            let out_b = run(b, tmpl, budget);
            assert_eq!(out_a, out_b, "tmpl={tmpl}, budget={budget}");
        }
    }
}
