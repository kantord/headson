# headson

Budget‑constrained, structure‑preserving previews for JSON files and streams — quick peeks that stay readable under a strict character budget.

Links: crates.io (Rust): https://crates.io/crates/headson · docs.rs: https://docs.rs/headson · PyPI (Python): https://pypi.org/project/headson/

## Install

Using Cargo:

    cargo install headson

From source:

    cargo build --release
    target/release/headson --help

Python (wheels for CPython 3.10–3.12 on Linux/macOS/Windows; others build from source):

    pip install headson

To build the Python module from source you need a Rust toolchain (and a recent pip/maturin). On Linux you may also need system build tools.

## Features

- Budgeted output: guarantees the preview stays within a chosen character budget.
- Three output styles: `json` (machine‑readable), `pseudo` (human‑friendly), `js` (JavaScript‑like).
- Multiple inputs: preview many files at once with a shared or per‑file budget.
- Fast: uses a streaming arena parser and SIMD‑accelerated JSON ingestion.
- CLI and libraries: usable from the terminal, Rust, and Python.

## Shell Mental Model

If you’re comfortable with tools like `head` and `tail`, use `headson` when you want a quick, structured peek into a JSON file without dumping the entire thing.

- `head` gives the first N lines/bytes, but it can break JSON in the middle of a token.
- `jq` prints valid JSON, but it prints the whole document unless you add filtering.
- `headson` picks a small but useful subset that fits a character budget and preserves structure, so you can scan the shape and some sample values quickly.

Why not `jq -c . file.json | head -c 200`?
- That can cut tokens mid‑way → invalid JSON or unreadable fragments.
- `headson` ensures the preview remains structured; arrays/objects may be truncated with ellipses/comments, but the result is still readable (and valid JSON when using the `json` template).

## Usage

    headson [FLAGS] [INPUT...]

- INPUT (optional, repeatable): file path(s). If omitted, reads JSON from stdin. Multiple input files are supported.
- Prints the preview to stdout. On parse errors, exits non‑zero and prints an error to stderr.

Common flags:

- `-n, --budget <BYTES>`: per‑file output budget. When multiple input files are provided, the total budget equals `<BYTES> * number_of_inputs`.
- `-N, --global-budget <BYTES>`: total output budget across all inputs. Useful when you want a fixed-size preview across many files (may omit entire files). Mutually exclusive with `--budget`.
- `-f, --template <json|pseudo|js>`: output style (default: `pseudo`)
- `-m, --compact`: no indentation, no spaces, no newlines
- `--no-newline`: single line output
- `--no-space`: no space after `:` in objects
- `--indent <STR>`: indentation unit (default: two spaces)
- `--string-cap <N>`: max graphemes to consider per string (default: 500)

Notes:

- With multiple input files:
  - JSON template outputs a single JSON object keyed by the input file paths.
  - Pseudo and JS templates render file sections with human-readable headers.
  - Using `--global-budget` may truncate or omit entire files to respect the total budget.
  - The tool finds the largest preview that fits the budget; if even the tiniest preview exceeds it, you still get a minimal, valid preview.
  - When passing file paths, directories and binary files are ignored; a notice is printed to stderr for each (e.g., `Ignored binary file: ./path/to/file`). Stdin mode reads the stream as-is.

Quick one‑liners:

- Peek a big JSON stream (keeps structure):

      zstdcat huge.json.zst | headson -n 800 -f pseudo

- Many files with a fixed overall size:

      headson -N 1200 -f json logs/*.json

- Glance at a file, JavaScript‑style comments for omissions:

      headson -n 400 -f js data.json

Examples:

- Read from stdin with defaults:

      cat data.json | headson

- Read from file, JS style, 200‑byte budget:

      headson -n 200 -f js data.json

- JSON style, compact:

      headson -f json -m data.json

- Multiple files (JSON template produces an object keyed by paths):

      headson -f json a.json b.json

- Global limit across files (fixed total size across all files):

      headson -N 400 -f json a.json b.json

Show help:

    headson --help

## Examples: head vs headson

Input:

```json
{"users":[{"id":1,"name":"Ana","roles":["admin","dev"]},{"id":2,"name":"Bo"}],"meta":{"count":2,"source":"db"}}
```

Naive cut (can break mid‑token):

```bash
jq -c . users.json | head -c 80
# {"users":[{"id":1,"name":"Ana","roles":["admin","dev"]},{"id":2,"name":"Bo"}],"me
```

Structured preview with headson (pseudo):

```bash
headson -n 120 -f pseudo users.json
# {
#   users: [
#     { id: 1, name: "Ana", roles: [ "admin", … ] },
#     …
#   ]
#   meta: { count: 2, … }
# }
```

Machine‑readable preview (json):

```bash
headson -n 120 -f json users.json
# {"users":[{"id":1,"name":"Ana","roles":["admin"]}],"meta":{"count":2}}
```

## Python Bindings

A thin Python extension module is available on PyPI as `headson`.

- Install: `pip install headson` (prebuilt wheels for CPython 3.10–3.12 on Linux/macOS/Windows). Older/newer Python versions may build from source if Rust is installed.
- API:
  - `headson.summarize(text: str, *, template: str = "pseudo", character_budget: int | None = None) -> str`
    - `template`: one of `"json" | "pseudo" | "js"`
    - `character_budget`: maximum output size in characters (default: 500)

Example:

```python
import json
import headson

data = {"foo": [1, 2, 3], "bar": {"x": "y"}}
preview = headson.summarize(json.dumps(data), template="json", character_budget=200)
print(preview)
```

Developer install for the Python module (requires Rust):

```
pipx install maturin
maturin develop -m pyproject.toml -r
```

Alternatively with `uv`:

```
uv add --dev maturin pytest
uv sync
uv run --no-sync maturin develop -r
uv run --no-sync pytest -q
```

Note: Wheels are currently built for specific CPython versions. Migrating to abi3 (stable ABI across Python 3.x) is being considered to broaden compatibility.

## Rust Library

You can also use `headson` as a Rust library.

```rust
use headson::{RenderConfig, OutputTemplate, PriorityConfig};

fn main() -> anyhow::Result<()> {
    let cfg = RenderConfig {
        template: OutputTemplate::Pseudo,
        indent_unit: "  ".into(),
        space: " ".into(),
        newline: "\n".into(),
    };
    let prio = PriorityConfig { max_string_graphemes: 500, array_max_items: 64 };
    let json = br#"{"a": [1, 2, 3], "b": {"c": 4}}"#.to_vec();
    let out = headson::headson(json, &cfg, &prio, 500)?;
    println!("{}", out);
    Ok(())
}
```

For multiple files, collect `(path, bytes)` pairs and call `headson::headson_many` with a global budget.

## Development

- Rust: `cargo test` runs unit and snapshot tests.
- Python: tests live in `tests_py/`. With `uv`: `uv run pytest -q`.
- Lint/format: `cargo clippy`, `cargo fmt`, and `ruff` for Python.

## Releases

Releases to crates.io and PyPI are automated via GitHub Actions:

- Versioning and release PRs are handled by `release-plz`.
- After a Rust release is created, Python wheels are built with `maturin` and published via PyPI Trusted Publishing (OIDC).
- The Rust crate and Python package share the same version to keep APIs in sync.

## License

MIT
