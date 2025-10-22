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

- INPUT (optional, repeatable): file path(s). If omitted, reads JSON from stdin. Currently only the first file is processed.
- Prints the preview to stdout. On parse errors, exits non‑zero and prints an error to stderr.

Common flags:

- `-n, --budget <BYTES>`: output byte budget (default: 500)
- `-f, --template <json|pseudo|js>`: output style (default: `pseudo`)
- `-m, --compact`: no indentation, no spaces, no newlines
- `--no-newline`: single line output
- `--no-space`: no space after `:` in objects
- `--indent <STR>`: indentation unit (default: two spaces)
- `--string-cap <N>`: max graphemes to consider per string (default: 500)

Examples:

- Read from stdin with defaults:

      cat data.json | headson

- Read from file, JS style, 200‑byte budget:

      headson -n 200 -f js data.json

- JSON style, compact:

      headson -f json -m data.json

Show help:

    headson --help
