# headson

CLI tool to read JSON from stdin, build a prioritized view of the data, and render a truncated-yet-informative representation to stdout across multiple formats.

This README documents the actual behavior verified in the repository, the overall architecture, how to use the CLI, and current limitations and performance notes.

## Overview

- Pipeline: parse_json (serde_json) → build_priority_queue → binary search best k → render directly from arena (Askama templates).
- Output formats (Askama templates in `templates/`): `json`, `pseudo`, `js`.
- Truncation is driven by a binary search over the number of included nodes; a render that fits within the given output-size budget is selected.
- Profiling (`--profile`) prints timings to stderr for parse, PQ build, and probes, plus PQ internals.

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
- `--profile`: print timing breakdowns to stderr (parse, PQ build, probes; plus PQ internals).

Exit codes and I/O:

- Reads stdin into a string; prints rendered output to stdout.
- On parse errors, exits non‑zero and prints an error to stderr.

## Architecture

High-level flow:

1) Parse: `serde_json::from_str` into `serde_json::Value`.
2) Priority queue build: single walk over the JSON to produce a flat list of items, cumulative scores, and per-node metrics.
3) Node selection by binary search: search k ∈ [1, total_nodes] for the largest k that renders within the output-size budget.
4) Inclusion marking: include nodes with `order_index < k` plus their full ancestor closure; compute omitted counts using original sizes.
5) Render: arena-backed serializer delegates arrays/objects to Askama templates and handles string truncation.

### Priority Queue and Scoring

Each node gets a cumulative score: `score = parent_score + 1 + node_penalty`.

- Arrays: for item at index i, `node_penalty = (i^3) * M` with `M = 1e12`, making all of item 0 preferable over any part of item 1, etc.
- Strings: characters are expanded as child nodes (via `unicode-segmentation` graphemes). For char at index i, `node_penalty = i + max(0, i − 20)^2`, favoring prefixes and discouraging scattered picks.
- Others: `node_penalty = 0` (depth contributes via the `+1` per level).

The PQ is implemented as “Vec + sort”: nodes are walked once, collected with scores, then stably sorted by ascending priority to assign a global `order_index`. Per-node metrics (`NodeMetrics`) capture `array_len`, `object_len`, and `string_len` as needed.

### Inclusion and Truncation

- Include all nodes where `order_index < k`, plus ensure all their ancestors are included (ancestor closure).
- Compute omitted counts per node using original lengths from `NodeMetrics` minus included children.

### Rendering

`RenderConfig` controls behavior:

- `template: OutputTemplate` — one of `Json`, `Pseudo`, `Js`.
- `indent_unit: String` — indent characters for each depth.
- `space: String` — either `" "` or `""`; applied after colons in objects only.
- `profile: bool` — enables stderr timing logs.

Template semantics (in `templates/`):

- `json`:
  - Always valid JSON when nothing is omitted (i.e., budget large enough). Empty containers render as `[]` / `{}` compactly.
  - When truncated, the current templates render C-style comments with omitted counts (e.g., `/* N more items */`). This makes truncated JSON output not strictly valid JSON.
- `pseudo`:
  - Uses ellipsis markers (`…`) for truncation; empty fully-truncated containers render single-line markers (`[ … ]`, `{ … }`).
- `js`:
  - Uses comments like `/* N more items */` and `/* empty */`; respects `--no-space` around braces for empty containers (e.g., `{/* empty */}`).

Additional details:

- Arrays never include a space after commas; objects apply `space` after colons.
- Strings are escaped via `serde_json`. When truncated, only a quoted kept prefix plus an ellipsis is rendered, ensuring prefix-only semantics.

## Testing

- End-to-end snapshots (`tests/e2e.rs`, fixtures in `tests/e2e_inputs/`): run across all templates and budgets `10, 100, 250, 1000, 10000`.
- JSON conformance suite (`JSONTestSuite/test_parsing/`, test driver in `tests/json_par_files.rs`):
  - `y_*.json`: parse with serde, run `headson -f json -n 10000`, re-parse stdout as JSON and deep-compare equality.
  - `n_*.json`: serde rejects and CLI must fail with non-zero exit and non-empty stderr.
- Unit snapshots for PQ and tree internals are in `src/snapshots/` and `tests/snapshots/`.

Run tests: `cargo test`.

## Performance and Profiling

Enable profiling with `--profile` to print timings to stderr, e.g.:

- PQ breakdown: `walk` (including string grapheme enumeration), `sort`, and `maps` (arena builds).
- Overall timings: `parse`, `pq`, `search+render`, and `total`.

Observed characteristics and current hotspots:

- String grapheme enumeration can dominate when there are many long strings.
- Building maps (`id_to_item`, `parent_of`, `children_of`, `order_index`) is a non‑trivial cost (now using Vecs to reduce overhead). Per‑node child sorting was removed.
- Rendering is typically negligible compared to PQ build and probe builds.

Recent optimizations implemented:
- PQ arena switched from HashMaps to Vecs (id-indexed), greatly reducing map overhead.
- Inclusion marking uses a reusable generational bitset across probes (no per-probe clears).
- Tree building switched to on-demand traversal from the arena (no per-probe filtered vectors).
- Removed per-node child sorting during tree build (children already ordered in PQ phase).
- String micro-optimizations: avoid per-grapheme string allocations; truncated strings use parent prefix slicing by grapheme count.
- QueueItem stores typed values (number/bool/string); removed reparsing and `value_repr`.

These changes cut PQ time and per-probe build time substantially on large inputs.

## Repository Layout

- `src/main.rs`: CLI argument parsing and I/O glue.
- `src/lib.rs`: public API, orchestration, binary search over k, and profiling output.
- `src/queue.rs`: PQ build, scoring, per-node metrics, and stable order assignment.
- `src/tree.rs`: arena-backed serializer, inclusion marking, and omitted-count logic.
- `src/render.rs`: Askama templates bindings and rendering helpers.
- `templates/`: Askama templates for `json`, `pseudo`, and `js`.
- `tests/`: E2E snapshots, JSON conformance tests, and fixtures.
- `JSONTestSuite/`: upstream test corpus used by the conformance tests.

## Known Issues and Limitations

- JSON template truncation is not valid JSON: when content is omitted, the `json` templates include comments like `/* N more items */`. Tests avoid this by using a large budget (`-n 10000`) for conformance. If strict JSON is required under truncation, the templates must be adjusted to use JSON-native markers (e.g., strings) instead of comments.
- Budget semantics: `-n/--budget` constrains the rendered output length in bytes, not the number of nodes. Internally we binary-search k (a node count) to fit the byte-length budget. Non-ASCII characters count by bytes, not grapheme clusters.
- Performance hotspots: long-string grapheme enumeration and HashMap/map builds dominate PQ time on large inputs.
- Dependencies pruned: removed unused `priority-queue` crate.
- Minor CLI polish: the `about` description is outdated relative to current functionality.
- Object key ordering follows `serde_json::Map` iteration order; stability can depend on the upstream map implementation and input.

## Installation

- Build from source: `cargo build --release`.
- Run: `cat input.json | target/release/headson [flags]`.

## Future Work

- Make truncated `json` output strict JSON (no comments) while still conveying omitted counts.
- Consider renaming `--budget` to clarify it is an output-size budget, or add a separate node-budget mode if needed.
- Explore faster string handling and/or configurable string expansion (e.g., cap grapheme enumeration).
- Reduce map-building costs; investigate arena representations that reduce allocations.
