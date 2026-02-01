# headson — AGENTS Guide

This is a distilled, action-oriented guide for agents working on `headson`. Use it to keep changes aligned with how the tool is meant to behave.

## Mission
- `headson` is a structure-aware head/tail for JSON, YAML, and text: given a byte/char/line budget, emit a compact preview that preserves tree shape and surfaces representative values (see `README.md`).
- Ships as a Rust CLI (`hson`) and ABI3 Python bindings (`headson.summarize(...)`) so the same renderer can be embedded in shells, scripts, or notebooks.

## Primary Surfaces to Mind
- CLI (`src/main.rs`): Clap parser for budgets, format/template/style selection, color controls, compact/no-newline toggles, head/tail skew, multi-input filesets. Detects binaries via `content_inspector`; prints stdout preview and stderr notices.
- Library (`src/lib.rs`): Entry `headson(InputKind, RenderConfig, PriorityConfig, Budgets) -> String`. `InputKind` covers `Json`, `Yaml`, `Text { mode }`, and multi-file `Fileset`. `TextMode` distinguishes plain text vs. code-like (indent-aware, atomic lines).
- Python bindings (`python/src/lib.rs`, `python/README.md`): `headson.summarize` mirrors CLI knobs. Wheels via `maturin`; parity tests in `tests_py/`.

## Core Pipeline (keep behaviors intact)
1. Ingest (`src/ingest`): Normalize into `JsonTreeArena` with flat node/child arrays + metadata for filesets and code-aware rendering. Formats:
   - JSON: `simd-json` + Serde `DeserializeSeed` builder (`ingest/formats/json`) that can pre-sample arrays.
   - YAML: `yaml-rust2` loader (`ingest/formats/yaml`) supporting multi-doc inputs (wrapped into arrays).
   - Text: heuristic builder (`ingest/formats/text`) that treats lines independently or, in `CodeLike`, builds indentation trees, keeps lines atomic, and stores the full line list for highlighting.
   - Filesets: `ingest/fileset.rs` merges per-file arenas under a synthetic object root; remembers filenames for section headers. Callers supply `FilesetInputKind` per file.
2. Priority Ordering (`src/order`): `build_order` scores nodes so top-level structure survives tight budgets. Arrays bias head/mid/tail per `PriorityConfig`; objects keep key order; strings expand by grapheme (capped via `max_string_graphemes`); filesets interleave round-robin; code-aware heuristics boost block-introducing lines and penalize blanks.
3. Budget Search (`src/pruner/budget.rs::find_largest_render_under_budgets`): Binary-search for the largest “top K nodes” that fit bytes/chars/lines. Measurements disable colors and optional fileset headers for exact counts (fileset-tree budgets are measured directly from the tree render; scaffolding lines are only charged when headers are charged). Once best K is found, ancestors/topology are reinserted. CLI `--debug` re-renders without color and emits JSON traces via `debug::emit_render_debug`.
4. Rendering (`src/serialization`): Templates for strict JSON, pseudo (`default`), JS-with-comments (`detailed`), YAML, raw Text, and Code (line numbers + syntect highlight). `Out` writer centralizes whitespace/newline/omission markers/colors/line-number gutters. Fileset helpers print section headers/summaries unless suppressed; omission markers vary by style.

## Heuristics & Semantics to Preserve
- Budgets: byte budgets default to 500 per input unless overridden; line/char budgets disable that implicit cap. Global vs per-file budgets apply simultaneously (`--bytes` + `--global-bytes`, `--lines` + `--global-lines`). `--head`/`--tail` influence array sampling and omission marker placement; strict JSON output stays unannotated. In tree mode, scaffold lines count toward budgets; under very tight global line budgets whole files may be omitted and surfaced as `… N more items` on the parent folders/root.
- Source code support: filename heuristics (`src/utils/extensions.rs`) decide “code mode” (atomic lines + indentation tree). Nested blocks omit from the middle to keep signatures; header lines get priority boosts. Stored `code_lines` snapshots allow syntax highlighting after sampling.
- Filesets: artificial object root enables per-file templates. Priority ordering interleaves nodes per file; renderers can inject headers unless `--no-header` is used. Inputs pre-sorted by frecency (git history via frecenfile) with mtime fallback; `--no-sort` keeps original order and skips repo scan.

## Testing & Tooling Expectations
- Rust: `cargo test` covers CLI flags, ingest edge cases, budgets, styles, filesets, debug traces, text/code heuristics, and snapshot-based end-to-end runs (`tests/e2e.rs`).
- Python: `uv run pytest -q` (or `maturin develop && pytest`) checks binding API (budget monotonicity, style annotations, skew behavior, strict JSON validity).
- Diagrams: `docs/diagrams/algorithm.mmd` is the source for the README SVG; regenerate with `cargo make diagrams`.
- Tooling: `Cargo.toml` enforces strict Clippy denies; release profile tuned for speed (Thin LTO, panic-abort, stripped symbols). Coverage artifacts like `lcov.info` are already gitignored. GitHub Actions are pinned to commit SHAs; Renovate keeps them current.

## Practical Notes for Agents
- When changing behavior, update `README.md`—library docs inline it via `#![doc = include_str!("../README.md")]` in `src/lib.rs`.
- CLI alias is `hson`; during dev use `cargo run -- <args>`. Keep stdout/stderr semantics consistent.
- Preserve tree shape and sampling guarantees before altering priorities or renderers; cross-check budget math and omission markers when tweaking templates.
- Run both Rust and Python suites after changes touching shared behavior to keep parity between CLI and bindings.
- Squash-style commit messages follow the repo’s pattern, e.g., `refactor: budget search and harden budget tests`; prefer format `<type>: <short description>`.
- Default to leaving git operations (commits, pushes) to the user unless explicitly asked; surface suggested commit messages when helpful.
