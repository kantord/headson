# headson

Budget‑constrained JSON preview for the terminal.

## Install

Using Cargo:

    cargo install headson

From source:

    cargo build --release
    target/release/headson --help

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

## Python package

Headson is also available as a Python extension module built with PyO3/maturin.

Install from PyPI:

    pip install headson

Example:

    import json
    import headson

    data = {"foo": [1, 2, 3], "bar": {"x": "y"}}
    preview = headson.summarize(json.dumps(data), template="json", character_budget=200)
    print(preview)
