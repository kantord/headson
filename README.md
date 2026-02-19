<h1 align="center">
  <img src="https://raw.githubusercontent.com/kantord/headson/main/docs/assets/logo.svg" alt="headson" width="221" />
</h1>

<p align="center">
  <a href="#quickstart">Quickstart</a> ·
  <a href="#common-problems">Common problems</a> ·
  <a href="#install">Install</a> ·
  <a href="#usage-reference">Usage reference</a> ·
  <a href="#python-bindings">Python bindings</a>
</p>

<p align="center">
<img src="https://raw.githubusercontent.com/kantord/headson/main/docs/assets/tapes/demo.gif" alt="Terminal demo" width="1560" height="900" />
  <br/>
</p>

## Your JSON file is too big to open. Now what?

`head -c` breaks your JSON mid-value. Your editor chokes on gigabyte files. You just want to see the shape of the data.

**headson** (`hson`) is a structure-aware `head`/`tail` for JSON, YAML, JSONL/NDJSON, and text. It parses the full tree, then produces a compact preview that preserves structure and representative values — all within a strict byte, character, or line budget.

Available as a CLI (`hson`) and a [Python library](#python-bindings) (`headson.summarize(...)`).

![Codecov](https://img.shields.io/codecov/c/github/kantord/headson?style=flat-square) ![Crates.io Version](https://img.shields.io/crates/v/headson?style=flat-square) ![PyPI - Version](https://img.shields.io/pypi/v/headson?style=flat-square)

## Quickstart

```bash
# Preview a big JSON file (default 500-byte budget)
hson data.json

# Get a valid, parseable JSON snippet under a byte budget
hson -c 500 -t strict data.json

# Preview JSONL / NDJSON logs (auto-detected from extension)
hson logs.jsonl

# Preview many files with a single total budget
hson -c 200 -C 2000 data/*.json configs/*.yaml

# Find errors in JSON logs — matching keys/values stay visible with full path context
hson --grep 'error|warning' -c 200 -C 2000 logs/*.json
```

<a id="common-problems"></a>

## Solve these common problems

### How do I preview a huge JSON file without crashing my editor?

```bash
hson data.json                  # 500-byte default preview
hson -c 2000 data.json          # larger preview
hson -n 30 data.json            # or cap at 30 lines
```

`hson` parses the file fully, scores every node by structural importance, then emits a compact preview that preserves the tree shape and surfaces representative values. You see the overall structure without loading the file into an editor.

### `head -c` breaks my JSON — how can I preview it safely?

`head -c` cuts at a raw byte offset, often mid-key or mid-value:

```bash
head -c 80 users.json
# {"users":[{"id":1,"name":"Ana","roles":["admin","dev"]},{"id":2,"name":"Bo"}],"me
```

`hson` respects structure — truncation never breaks a key or value:

```bash
hson -c 120 users.json
# {
#   "meta": { "count": 2, ... }
#   "users": [
#     { "id": 1, "name": "Ana", "roles": [ "admin", ... ] }
#     ...
#   ]
# }
```

### How do I get a valid JSON snippet from a giant file for testing?

```bash
hson -c 500 -t strict data.json
```

`-t strict` produces valid, parseable JSON — no omission markers, no comments. Pruned children become empty objects/arrays; long strings are truncated. Pipe it to `jq` or use it as test fixture input.

```bash
# Confirm it's valid JSON
hson -c 500 -t strict data.json | jq .
```

### How do I preview JSONL / NDJSON logs quickly?

```bash
# From a file (auto-detected from .jsonl or .ndjson extension)
hson logs.jsonl

# From stdin (specify -i jsonl)
cat stream.jsonl | hson -c 1000 -i jsonl

# Show the tail end of logs
hson -c 500 --tail logs.jsonl
```

JSONL entries are displayed with their original line numbers. Under tight budgets, `hson` samples from the head, middle, and tail of the file so you see representative entries from across the log.

### How do I grep JSON but keep matching paths/parents visible?

```bash
hson --grep 'error' -c 500 data.json
```

`--grep` guarantees that matching values (and their ancestor keys) stay in the output, even under tight budgets. Everything else fills the remaining space.

```bash
# Multiple patterns (OR)
hson --grep 'error' --grep 'warning' logs.json

# Case-insensitive
hson --igrep 'error' logs.json

# Search across many files — only files with matches are shown
hson --grep 'error' -c 200 -C 2000 logs/*.json

# Soft bias (prioritize matches, but don't guarantee them)
hson --weak-grep 'important' -c 200 data.json
```

![Grep demo](https://raw.githubusercontent.com/kantord/headson/main/docs/assets/tapes/grep.gif)

### How do I preview many JSON/YAML files at once with a single total budget?

```bash
# Per-file cap + global cap
hson -c 200 -C 2000 data/*.json configs/*.yaml

# Or use globs (respects .gitignore)
hson --glob 'src/**/*.json' --glob 'config/**/*.yaml' -c 200 -C 2000
```

In a git repo, files are automatically sorted by frecency (frequently + recently touched files first). Each file gets a fair share of the budget via round-robin selection.

![Sorting demo](https://raw.githubusercontent.com/kantord/headson/main/docs/assets/tapes/sort.gif)

### How do I get a tree view of a directory with inline previews?

```bash
hson --tree -r -c 200 -C 2000 src/
# or with globs
hson --tree --glob 'src/**/*' -c 200 -C 2000
```

Renders a directory tree with inline structured previews per file. Code files get line numbers and syntax highlighting; JSON/YAML files show their tree structure.

![Tree demo](https://raw.githubusercontent.com/kantord/headson/main/docs/assets/tapes/tree.gif)

### How do I make an LLM-friendly preview of huge JSON/YAML under a strict budget?

```bash
# Strict JSON for machine consumption (~500 bytes ≈ 125–250 tokens)
hson -c 2000 -t strict data.json

# Default style for human-readable LLM context (... markers show where content was trimmed)
hson -c 4000 data.json

# Repo snapshot for AI code review
hson --tree -r -c 100 -C 4000 .
```

Byte budgets map roughly to token budgets (1 byte ≈ 0.25–0.5 tokens for English/code). Use `-t strict` when the LLM needs to parse the output as JSON; use the default style when it just needs readable context.

In Python:

```python
import headson
preview = headson.summarize(huge_json_string, byte_budget=2000, style="strict")
```

## Features

- **Budgeted output**: bytes (`-c`), characters (`-u`), or lines (`-n`); per-file and global caps
- **Output formats**: `auto | json | yaml | text` with styles `strict | default | detailed`
- **Structure-aware**: full JSON/YAML/JSONL parsing preserves tree shape under truncation
- **Source code support**: indentation-aware outlines with syntax highlighting for 130+ languages
- **Multi-file mode**: paths, `--glob`, or `--recursive`; shared or per-file budgets
- **Repo-aware ordering**: frequently + recently touched files surface first (git history + mtime fallback)
- **Grep**: `--grep <regex>` guarantees matching values stay visible with ancestor context
- **Tree view**: `--tree` renders a directory tree with inline previews
- **JSONL/NDJSON**: auto-detected from extension; line numbers preserved in output
- **Python library**: `pip install headson` for the same engine in scripts and notebooks

### Source code mode

For source code files, headson uses an indentation-aware heuristic to build an outline, then picks representative lines (keeping lines atomic — omissions never split a line). Syntax highlighting is available when colors are enabled.

```bash
hson -n 20 src/main.py
```

![Code demo](https://raw.githubusercontent.com/kantord/headson/main/docs/assets/tapes/code.gif)

## Install

Using Cargo:

```sh
cargo install headson
```

> Note: the package is called `headson`, but the installed CLI command is `hson`. All examples use `hson ...`.

From source:

```sh
cargo build --release
target/release/hson --help
```

Shell completions:

```sh
hson --completions bash > ~/.local/share/bash-completion/completions/hson
hson --completions zsh > ~/.zsh/completions/_hson
hson --completions fish > ~/.config/fish/completions/hson.fish
```

## Usage reference

```text
hson [FLAGS] [INPUT...]
```

- `INPUT` (optional, repeatable): file path(s). If omitted, reads from stdin.
- Prints the preview to stdout. On parse errors, exits non-zero with an error on stderr.

### Common flags

- `-c, --bytes <BYTES>`: per-file output budget (bytes). Default is 500 when no budget is set. For multiple inputs, default total is `<BYTES> * input_count`.
- `-u, --chars <CHARS>`: per-file output budget (Unicode code points). Behaves like `--bytes` but counts characters.
- `-n, --lines <LINES>`: per-file line budget. Add `--global-lines` for an aggregate cap.
- `-C, --global-bytes <BYTES>`: total byte budget across all inputs. With `--bytes`, the effective total is the smaller of the two.
- `-N, --global-lines <LINES>`: total line budget across all inputs.
- `-H, --count-headers`: count fileset headers/summary lines toward budgets (they're free by default).
- `-f, --format <auto|json|yaml|text>`: output format (default: `auto`).
  - Auto: stdin defaults to JSON; files are detected by extension (`.json` → JSON, `.yaml`/`.yml` → YAML, `.jsonl`/`.ndjson` → JSONL, others → Text).
- `-t, --template <strict|default|detailed>`: output style (default: `default`).
  - `strict`: valid JSON/YAML with no omission markers.
  - `default`: human-readable with `...` omission markers.
  - `detailed`: JS-style comments (`/* N more items */`) or YAML comments (`# N more items`).
- `-i, --input-format <json|jsonl|yaml|text>`: ingestion format override (default: `json` for stdin; auto-detected for files).
- `-m, --compact`: no indentation, no spaces, no newlines.
- `--no-newline`: single-line output. Incompatible with `--lines`/`--global-lines`.
- `--head`: prefer the beginning of arrays when truncating. Mutually exclusive with `--tail`.
- `--tail`: prefer the end of arrays when truncating. Mutually exclusive with `--head`.
- `--string-cap <N>`: max graphemes per string (default: 500).
- `--color` / `--no-color`: force enable/disable ANSI colors (default: auto-detect TTY).
- `--indent <STR>`: indentation unit (default: two spaces).

### Multi-file mode

- Budgets: per-file caps apply to each input; global caps (`--global-bytes`/`--global-lines`) constrain the combined output. Default byte budgets scale by input count when no globals are set.
- One metric per level: pick at most one per-file budget (`--bytes` | `--chars` | `--lines`) and at most one global (`--global-bytes` | `--global-lines`). Mixing per-file and global kinds is allowed (e.g., `-n 3 -C 120`).
- Inputs: pass file paths directly, use `--glob <PATTERN>` to expand files (respects `.gitignore`), or `--recursive` to expand directories (incompatible with `--glob`).
- Sorting: inputs are ordered by frecency (git history + mtime fallback). Pass `--no-sort` to keep input order and skip repo scanning.
- Headers: multi-file output gets `==> filename <==` headers; hide with `--no-header`. Compact/single-line modes omit headers automatically.
- Formats: in `--format auto`, each file picks its renderer by extension. Unknown extensions fall back to Text.
- Fairness: file contents are interleaved round-robin so tight budgets don't starve later files.
- Parse failures: reported on stderr; the file renders as a header with an empty body.
- Binary files: detected and skipped with a stderr warning.

### Grep mode

Use `--grep <REGEX>` to guarantee that matching values/keys/lines (and their ancestors) appear in the output. Budgets apply to everything else.

- `--igrep <REGEX>`: case-insensitive variant.
- `--weak-grep <REGEX>` / `--weak-igrep <REGEX>`: bias priority toward matches without guaranteeing inclusion. Combinable with strong grep.
- Multiple patterns: all grep flags are repeatable with OR semantics (`--grep foo --grep bar`).
- Multi-file filtering: by default (`--grep-show matching`), files without matches are dropped. Use `--grep-show all` to keep them.
- Colors: only matching text is highlighted; syntax colors are suppressed in grep mode.
- Context: no `-C/-B/-A` flags; per-file budgets decide how much surrounding structure stays alongside must-keep matches.

### Tree mode

Use `--tree` for a directory tree layout with inline previews instead of `==>` headers.

- Layout: classic tree branches with continuous guides; code line numbers stay visible.
- Budgets: tree scaffolding is free by default (set `--count-headers` to charge it). Tight global caps can omit entire files (`... N more items`).
- Sorting: respects `--no-sort`; otherwise uses frecency ordering before tree grouping.
- Compatible with grep: matches are shown inside the tree.

### Budget modes

- **Bytes** (`-c`/`-C`): measures UTF-8 bytes. Default per-file budget is 500 bytes when no other budget is set.
- **Characters** (`-u`): measures Unicode code points.
- **Lines** (`-n`/`-N`): caps line count. Headers don't count unless `-H` is set. Incompatible with `--no-newline`.
- All active budgets are enforced simultaneously; the strictest cap wins.
- When only lines are specified, no implicit byte cap applies.

### Text and source code

- Text files are detected by extension (anything not JSON/YAML/JSONL). Force with `-i text -f text`.
- Source code files (~130 extensions) get indentation-aware outlines: block-introducing lines (function/class headers) are prioritized; omissions never split a line.
- With colors enabled, source code gets syntax highlighting and line numbers. Gaps in line numbers signal omitted blocks.
- Styles: `default` shows `...` markers, `detailed` shows `... N more lines ...`, `strict` omits markers entirely.

## How it compares

- **`head`/`tail`**: byte/line-based truncation that breaks JSON/YAML structure.
- **`jq`**: powerful query language, but you need to write filters to get a compact preview. `hson` is zero-config.
- **`fx`/`jless`**: interactive TUI browsers for exploration. `hson` is non-interactive — designed for piping, scripting, and strict output budgets.

## Python Bindings

A thin Python extension module on PyPI.

<a id="python-bindings-install"></a>

### Install

```sh
pip install headson
```

ABI3 wheels for Python 3.10+ on Linux/macOS/Windows.

<a id="python-bindings-usage"></a>

### Usage

```python
headson.summarize(
    text: str,
    *,
    format: str = "auto",         # "auto" | "json" | "yaml" | "text" | "code"
    style: str = "default",       # "strict" | "default" | "detailed"
    input_format: str = "json",   # "json" | "jsonl" | "ndjson" | "yaml" | "text"
    byte_budget: int | None = None,  # default: 500 bytes
    skew: str = "balanced",       # "balanced" | "head" | "tail"
    grep: str | None = None,      # regex for guaranteed inclusion
    weak_grep: str | None = None, # regex for priority bias (no guarantee)
) -> str
```

- `style="strict"` produces valid JSON/YAML with no markers.
- `grep` guarantees matching values appear; prefix with `(?i)` for case-insensitive.
- Colors are always off in the Python API.
- Fileset/tree mode is not available (single input only).

### Examples

```python
import json
import headson

# Valid JSON snippet
data = {"foo": [1, 2, 3], "bar": {"x": "y"}}
preview = headson.summarize(json.dumps(data), format="json", style="strict", byte_budget=200)
print(preview)

# Tail of a large array with detailed comments
print(
    headson.summarize(
        json.dumps(list(range(100))),
        format="json",
        style="detailed",
        byte_budget=80,
        skew="tail",
    )
)

# YAML
doc = "root:\n  items: [1,2,3,4,5,6,7,8,9,10]\n"
print(headson.summarize(doc, format="yaml", style="default", input_format="yaml", byte_budget=60))

# JSONL
logs = '{"level":"info","msg":"ok"}\n{"level":"error","msg":"fail"}\n'
print(headson.summarize(logs, input_format="jsonl", byte_budget=200))

# Grep — guarantee a match appears
print(headson.summarize('{"needle":123,"other":456}', grep="needle"))
```

# Algorithm

![Algorithm overview](https://raw.githubusercontent.com/kantord/headson/main/docs/assets/algorithm.svg)

## Footnotes
 - <sup><b>[1]</b></sup> <b>Optimized tree representation</b>: An arena-style tree stored in flat, contiguous buffers. Each node records its kind and value plus index ranges into shared child and key arrays. Arrays are ingested in a single pass and may be deterministically pre-sampled: the first element is always kept; additional elements are selected via a fixed per-index inclusion test; for kept elements, original indices are stored and full lengths are counted. This enables accurate omission info and internal gap markers later, while minimizing pointer chasing.
 - <sup><b>[2]</b></sup> <b>Priority order</b>: Nodes are scored so previews surface representative structure and values first. Arrays can favor head/mid/tail coverage (default) or strictly the head; tail preference flips head/tail when configured. Object properties are ordered by key, and strings expand by grapheme with early characters prioritized over very deep expansions.
 - <sup><b>[3]</b></sup> <b>Choose top N nodes (binary search)</b>: Iteratively picks N so that the rendered preview fits within the byte budget, looping between "choose N" and a render attempt to converge quickly.
 - <sup><b>[4]</b></sup> <b>Render attempt</b>: Serializes the currently included nodes using the selected template. Omission summaries and per-file section headers appear in display templates (pseudo/js); json remains strict. For arrays, display templates may insert internal gap markers between non-contiguous kept items using original indices.
 - <sup><b>[5]</b></sup> <b>Diagram source</b>: The Algorithm diagram is generated from `docs/diagrams/algorithm.mmd`. Regenerate the SVG with `cargo make diagrams` before releasing.

## License

MIT


[![Star History Chart](https://api.star-history.com/svg?repos=kantord/headson&type=date&legend=top-left)](https://www.star-history.com/#kantord/headson&type=date&legend=top-left)
