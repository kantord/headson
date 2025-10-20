# headson

CLI tool to read JSON from stdin, build a prioritized view of the data, and render a truncated-yet-informative representation to stdout across multiple formats.

This README documents the actual behavior verified in the repository, the overall architecture, how to use the CLI, and current limitations and performance notes.

## Overview

- Pipeline: parse_json (simd-json via serde bridge) → build_priority_order → binary search best k → render directly from arena with a code-based renderer.
- Output formats: `json`, `pseudo`, `js` (selected via a style switch in the renderer).
- Truncation is driven by a binary search over the number of included nodes; a render that fits within the given output-size budget is selected.
- Profiling (`--profile`) prints timings to stderr for parse, priority-order build, and probes, plus internal stats.

## CLI

Usage examples:

- `cat data.json | headson` → default template `pseudo`, default budget 500, two-space indent, space after colons.
- `cat data.json | headson -n 200 -f js` → limit output to ~200 bytes and use the JS-style template.
- `cat data.json | headson -f json --no-space --indent "\t"` → JSON template, tabs for indent, no space after colons in objects.

Flags:

- `-n, --budget <int>`: output size budget in bytes. The program binary-searches the largest node count k whose rendered string length `s.len()` (bytes) is `<= budget`.
- `-f, --template <json|pseudo|js>`: output format/template.
- `--indent <string>`: indentation unit (default: two spaces).
- `--no-space`: remove the single space after `:` in objects. Arrays never add spaces after commas.
- `--no-newline`: remove newlines from output (one-line rendering).
- `-m, --compact`: compact output (no indentation, no spaces after colons, no newlines). Conflicts with `--indent`, `--no-space`, and `--no-newline`.
- `--profile`: print timing breakdowns to stderr (parse, priority-order build, probes; plus internals).
- `--string-cap <int>`: maximum graphemes to expand per string during priority-order build (default: 500). Caps work on long strings.
 - `--input <path>`: read JSON directly from a file instead of stdin.

Exit codes and I/O:

- Reads stdin into a string; prints rendered output to stdout.
- On parse errors, exits non‑zero and prints an error to stderr.

## Architecture

High-level flow:

1) Parse: `simd_json::serde::from_slice` into `serde_json::Value` (Stage 1 swap for faster parsing).
2) Priority order build (frontier): best‑first (min‑heap) expansion by cumulative score; builds just enough nodes for probing (no global full‑build/sort). Per‑node metrics capture sizes/truncation flags.
3) Node selection by binary search: search k ∈ [1, total_nodes] for the largest k that renders within the output-size budget.
4) Inclusion marking: include nodes with `order_index < k` plus their full ancestor closure; compute omitted counts using original sizes.
5) Render: arena-backed serializer writes arrays/objects with a small style strategy and handles string truncation.

### Priority Order and Scoring

Each node gets a cumulative score: `score = parent_score + 1 + node_penalty`.

- Arrays: for item at index i, `node_penalty = (i^3) * M` with `M = 1e12`, making all of item 0 preferable over any part of item 1, etc.
- Strings: characters are expanded as child nodes (via `unicode-segmentation` graphemes). For char at index i, `node_penalty = i + max(0, i − 20)^2`, favoring prefixes and discouraging scattered picks.
- Others: `node_penalty = 0` (depth contributes via the `+1` per level).

Frontier priority-order build (default): a best‑first traversal yields `ids_by_order` directly without sorting all nodes. Per‑node metrics capture `array_len`, `object_len`, and `string_len` or `string_truncated` as needed.

### Inclusion and Truncation

- Include all nodes where `order_index < k`, plus ensure all their ancestors are included (ancestor closure).
- Compute omitted counts per node using original lengths from `NodeMetrics` minus included children.

### Rendering

`RenderConfig` controls behavior:

- `template: OutputTemplate` — one of `Json`, `Pseudo`, `Js`.
- `indent_unit: String` — indent characters for each depth.
- `space: String` — either `" "` or `""`; applied after colons in objects only.
- `newline: String` — either `"\n"` or `""`; applied as a post-process replacement of default newlines.
- `profile: bool` — enables stderr timing logs.

Rendering semantics by style:

- `json`:
  - Always valid JSON when nothing is omitted (i.e., budget large enough). Empty containers render as `[]` / `{}` compactly.
  - When truncated, the current templates render C-style comments with omitted counts (e.g., `/* N more items */`). This makes truncated JSON output not strictly valid JSON.
- `pseudo`:
  - Uses ellipsis markers (`…`) for truncation; empty fully-truncated containers render single-line markers (`[ … ]`, `{ … }`).
- `js`:
  - Uses comments like `/* N more items */` and `/* empty */`; respects `--no-space` around braces for empty containers (e.g., `{/* empty */}`).

Additional details:

- Arrays never include a space after commas; objects apply `space` after colons.
- Strings are escaped via `serde_json`. When truncated, only a quoted kept prefix plus an ellipsis is rendered, ensuring prefix‑only semantics.

### Order Caps (configurable)

- `--string-cap <n>`: hard cap on grapheme expansion per string during priority-order build (default: 500). Prevents runaway work for very long strings. Rendering still shows prefix + ellipsis.
- Array cap (derived from budget): per‑array expansion is capped at `budget / 2`, based on a conservative lower bound that an array of N items needs ~2N characters to fit. Arrays longer than this cannot fit within the budget, so we avoid walking/pushing those extra items during the priority‑order build.

## Testing

- End-to-end snapshots (`tests/e2e.rs`, fixtures in `tests/e2e_inputs/`): run across all templates and budgets `10, 100, 250, 1000, 10000`.
- JSON conformance suite (`JSONTestSuite/test_parsing/`, test driver in `tests/json_parse_files.rs`):
  - `y_*.json`: parse with serde, run `headson -f json -n 10000`, re-parse stdout as JSON and deep-compare equality.
  - `n_*.json`: serde rejects and CLI must fail with non-zero exit and non-empty stderr.
- Unit snapshots for order and tree internals are in `src/snapshots/` and `tests/snapshots/`.

Run tests: `cargo test`.

## Performance and Profiling

Enable profiling with `--profile` to print timings to stderr, e.g.:

- Order build breakdown: `walk` (including string grapheme enumeration) and `maps` (arena builds).
- Overall timings: `parse`, `order`, `search+render`, and `total`.

Observed characteristics and current hotspots:

- String grapheme enumeration can dominate when there are many long strings.
- Building maps (`id_to_item`, `parent_of`, `children_of`, `order_index`) is a non‑trivial cost (now using Vecs to reduce overhead). Per‑node child sorting was removed.
- Rendering is typically negligible compared to the priority-order build and probe builds.

Recent optimizations implemented:
- Frontier (top‑K) priority-order build (default in CLI): best‑first expansion by cumulative score (no full sort/build).
- O(k) inclusion marking via `ids_by_order` for probes.
- Arena-backed render (no intermediate tree allocations).
- Order arena switched from HashMaps to Vecs (id-indexed).
- Removed per-node child sorting (children already ordered in PQ phase).
- String cap and micro‑opts (no per‑grapheme allocations; prefix slicing by grapheme count).
- RankedNode stores typed values (number/bool/string); removed reparsing and `value_repr`.

Older optimizations:
- Order arena switched from HashMaps to Vecs (id-indexed), greatly reducing map overhead.
- Inclusion marking uses a reusable generational bitset across probes (no per-probe clears).
- Tree building switched to on-demand traversal from the arena (no per-probe filtered vectors).
- Removed per-node child sorting during tree build (children already ordered in PQ phase).
- String micro-optimizations: avoid per-grapheme string allocations; truncated strings use parent prefix slicing by grapheme count.
- RankedNode stores typed values (number/bool/string); removed reparsing and `value_repr`.

These changes cut priority‑order build time and per-probe build time substantially on large inputs.

## Repository Layout

- `src/main.rs`: CLI argument parsing and I/O glue.
- `src/lib.rs`: public API, orchestration, binary search over k, and profiling output.
- `src/order.rs`: Priority order build, scoring, per-node metrics, and stable order assignment.
- `src/tree.rs`: arena-backed serializer, inclusion marking, and omitted-count logic.
- `src/render.rs`: code-based renderer with style switches for `json`, `pseudo`, and `js`.
- `tests/`: E2E snapshots, JSON conformance tests, and fixtures.
- `JSONTestSuite/`: upstream test corpus used by the conformance tests.

## Benchmarking (Hyperfine)

For quick end-to-end timing, use Hyperfine. The script builds the release binary, generates synthetic data, and runs two scenarios to highlight I/O impact.

Requirements: hyperfine installed (https://github.com/sharkdp/hyperfine)

Default run (no args):

    bash scripts/bench_hyperfine.sh

What it measures (for multiple dataset sizes):

- Scales: runs 1×, 10×, 100× of `HF_GEN_COUNT` (default base 200,000 → 200k, 2M, 20M items).
- PIPE: generator → headson (live pipeline). Measures generator + parse/order/render.
- FILE: generator → file → headson --input file. Measures parse/order/render + disk read.
- GEN: generator → /dev/null. Measures pure generator cost.
- WRITE: generator → file. Measures write throughput (page-cache affected).

Outputs:

- Results saved under `benchmarks/hyperfine/bench_*.json` (per dataset size and scenario).
- The script prints each generated file's size and, if `jq` is available, a short summary comparing PIPE vs FILE per budget (mean ms and delta) for each dataset.

Tuning:

- Change base input size/seed: `HF_GEN_COUNT=500000 HF_GEN_SEED=123 bash scripts/bench_hyperfine.sh`
- Change budgets/template: `HF_BUDGETS=100,1000,10000 HF_TEMPLATE=pseudo bash scripts/bench_hyperfine.sh`
- Change dataset scales: `HF_SCALES=1,5,20 bash scripts/bench_hyperfine.sh`
- Stability: increase warmups or pin to a core: `HF_WARMUP=5 taskset -c 0 bash scripts/bench_hyperfine.sh`

## Known Issues and Limitations

- JSON template output is always valid JSON; when truncated, omitted entries/properties are simply not annotated in the JSON output (pseudo/js still show comments).
- Budget semantics: `-n/--budget` constrains the rendered output length in bytes, not the number of nodes. Internally we binary-search k (a node count) to fit the byte-length budget. Non-ASCII characters count by bytes, not grapheme clusters.
- Performance hotspots: parsing dominates on multi‑GB inputs (now using simd‑json’s serde bridge). Frontier PQ + caps keep PQ cost relatively small.
- simd‑json serde differences: a few edge cases in the JSONTestSuite differ from serde_json (e.g., handling of certain malformed numbers and signed zeros). Tests skip these; see `tests/json_parse_files.rs`.
- Dependencies pruned: removed unused `priority-queue` crate.
 
- Object key ordering follows `serde_json::Map` iteration order; stability can depend on the upstream map implementation and input.

## Installation

- Build from source: `cargo build --release`.
- Run: `cat input.json | target/release/headson [flags]`.

## Pre-commit Hooks (Formatting & Linting)

This repo ships a pre-commit configuration to enforce Rust formatting and Clippy lint checks (cognitive complexity) on commits.

Setup:

- Ensure rustfmt is installed: `rustup component add rustfmt`
- Ensure clippy is installed: `rustup component add clippy`
- Install pre-commit (Python): `pip install pre-commit`
- Install the Git hook in this repo: `pre-commit install`

What it does:

- Runs `cargo clippy` with cognitive complexity enabled and treats warnings as errors:
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings -W clippy::cognitive_complexity`
  - The threshold is configured in `.clippy.toml` (`cognitive-complexity-threshold = 23`).
- Runs `cargo fmt --all -- --check` before each commit and blocks if formatting differs.

Manual runs:

- Auto-fix formatting via pre-commit (manual stage):
  - `pre-commit run rustfmt-fix --all-files --hook-stage manual`
- Run clippy across the workspace:
  - `pre-commit run clippy --all-files`
- Run the check across the repo:
  - `pre-commit run rustfmt --all-files`
- Or run directly with Cargo:
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings -W clippy::cognitive_complexity`

Notes:
- The fix hook is configured with `stages: [manual]`, so it only runs when invoked with `--hook-stage manual`.
- After auto-fix, re-stage files (`git add -A`) and re-run the check if committing immediately.

## Future Work

- Make truncated `json` output strict JSON (no comments) while still conveying omitted counts.
- Consider renaming `--budget` to clarify it is an output-size budget, or add a separate node-budget mode if needed.
- Explore faster string handling and/or configurable string expansion (e.g., cap grapheme enumeration).
- Optional early-stop writer for probe renders; flat edge arena (single children buffer + offsets); streaming/partial deserialization (e.g., custom Visitor to cap arrays during deserialization), or faster parsers (e.g., simd‑json) for parse-bound workloads.
