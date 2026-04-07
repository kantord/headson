mod common;

fn run(input: &str, template: &str, budget: usize) -> String {
    common::run_template_budget_no_color(input, template, budget, &[])
}

#[test]
fn object_key_order_preserves_source_order() {
    // Keys should appear in the same order as in the source document.
    for &tmpl in &["json", "pseudo", "js"] {
        let abc = r#"{"a":1,"b":2,"c":0}"#;
        let out = run(abc, tmpl, 1000);
        let pa = out.find("\"a\"").unwrap();
        let pb = out.find("\"b\"").unwrap();
        let pc = out.find("\"c\"").unwrap();
        assert!(
            pa < pb && pb < pc,
            "template={tmpl}: source order a,b,c should be preserved: {out:?}"
        );

        let cba = r#"{"c":0,"b":2,"a":1}"#;
        let out2 = run(cba, tmpl, 1000);
        let pc2 = out2.find("\"c\"").unwrap();
        let pb2 = out2.find("\"b\"").unwrap();
        let pa2 = out2.find("\"a\"").unwrap();
        assert!(
            pc2 < pb2 && pb2 < pa2,
            "template={tmpl}: source order c,b,a should be preserved: {out2:?}"
        );
    }
}

#[test]
fn object_key_order_source_order_under_truncation() {
    // Under truncation, whichever keys fit should appear in source order.
    for &tmpl in &["json", "pseudo", "js"] {
        for &budget in &[10usize, 30usize, 60usize] {
            let abc = r#"{"a":1,"b":2,"c":0,"d":3}"#;
            let out = run(abc, tmpl, budget);
            // Collect positions of keys that appear in the output.
            let positions: Vec<(usize, usize)> = ["\"a\"", "\"b\"", "\"c\"", "\"d\""]
                .iter()
                .enumerate()
                .filter_map(|(i, key)| out.find(key).map(|pos| (i, pos)))
                .collect();
            // Verify that whatever subset appears maintains their original relative order.
            for w in positions.windows(2) {
                assert!(
                    w[0].1 < w[1].1,
                    "template={tmpl}, budget={budget}: source order not preserved; \
                     key[{}] at {} should be before key[{}] at {} in: {out:?}",
                    w[0].0, w[0].1, w[1].0, w[1].1
                );
            }
        }
    }
}
