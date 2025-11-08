use assert_cmd::cargo::cargo_bin_cmd;
use std::collections::HashSet;

struct FixtureExpectation {
    path: &'static str,
    min_unique_colors: usize,
    min_multi_color_lines: usize,
}

const FIXTURES: &[FixtureExpectation] = &[
    FixtureExpectation {
        path: "tests/fixtures/code/sample.py",
        min_unique_colors: 5,
        min_multi_color_lines: 5,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/sample.ts",
        min_unique_colors: 6,
        min_multi_color_lines: 5,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/Sample.tsx",
        min_unique_colors: 5,
        min_multi_color_lines: 4,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/sample.js",
        min_unique_colors: 5,
        min_multi_color_lines: 5,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/sample.go",
        min_unique_colors: 5,
        min_multi_color_lines: 5,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/sample.sh",
        min_unique_colors: 3,
        min_multi_color_lines: 3,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/sample.cpp",
        min_unique_colors: 4,
        min_multi_color_lines: 4,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/Sample.java",
        min_unique_colors: 4,
        min_multi_color_lines: 4,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/big_sample.py",
        min_unique_colors: 3,
        min_multi_color_lines: 3,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/minimal_drop_case.py",
        min_unique_colors: 3,
        min_multi_color_lines: 3,
    },
    FixtureExpectation {
        path: "tests/fixtures/code/multi_describe.test.js",
        min_unique_colors: 5,
        min_multi_color_lines: 6,
    },
];

fn run_colored_output(path: &str) -> String {
    let output = cargo_bin_cmd!("headson")
        .args(["--color", "-c", "400", "-f", "auto", path])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).expect("headson output should be UTF-8")
}

fn parse_ansi_sequences(line: &str) -> Result<Vec<String>, String> {
    let bytes = line.as_bytes();
    let mut sequences = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if i + 1 >= bytes.len() || bytes[i + 1] != b'[' {
                return Err(format!(
                    "invalid ANSI sequence prefix near byte {i}"
                ));
            }
            i += 2;
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_digit() || bytes[i] == b';')
            {
                i += 1;
            }
            if start == i {
                return Err(format!(
                    "empty ANSI parameters near byte {start}"
                ));
            }
            if i >= bytes.len() || bytes[i] != b'm' {
                return Err(format!(
                    "ANSI sequence not terminated with 'm' near byte {i}"
                ));
            }
            sequences.push(line[start..i].to_string());
            i += 1;
            continue;
        }
        i += 1;
    }
    Ok(sequences)
}

fn count_line_multi_colors(output: &str) -> Result<usize, String> {
    let mut count = 0usize;
    for raw_line in output.lines() {
        let Some((_, body)) = raw_line.split_once(':') else {
            continue;
        };
        let body = body.trim_start();
        if body.is_empty() {
            continue;
        }
        let seqs = parse_ansi_sequences(body)?;
        let unique: HashSet<_> =
            seqs.into_iter().filter(|seq| seq != "0").collect();
        if unique.len() >= 2 {
            count += 1;
        }
    }
    Ok(count)
}

#[test]
fn code_fixtures_emit_valid_ansi_and_color_variety() {
    for fixture in FIXTURES {
        let out = run_colored_output(fixture.path);
        let sequences = parse_ansi_sequences(&out)
            .unwrap_or_else(|e| panic!("{}: invalid ANSI: {e}", fixture.path));
        let unique_colors: HashSet<_> =
            sequences.into_iter().filter(|seq| seq != "0").collect();
        assert!(
            unique_colors.len() >= fixture.min_unique_colors,
            "fixture {} produced only {} distinct colors (expected at least {})",
            fixture.path,
            unique_colors.len(),
            fixture.min_unique_colors
        );

        let multi = count_line_multi_colors(&out).unwrap_or_else(|e| {
            panic!("{}: invalid ANSI per line: {e}", fixture.path)
        });
        assert!(
            multi >= fixture.min_multi_color_lines,
            "fixture {} had {multi} lines with >=2 colors (expected at least {})",
            fixture.path,
            fixture.min_multi_color_lines
        );
    }
}
