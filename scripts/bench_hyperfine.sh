#!/usr/bin/env bash
set -euo pipefail

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine not found. Install: https://github.com/sharkdp/hyperfine" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# Inputs (can be overridden via env or args)
INPUT_PATH="${1:-${HF_INPUT:-tests/e2e_inputs/complex_nested.json}}"
BUDGETS_STR="${HF_BUDGETS:-10,100,1000,10000}"
TEMPLATE="${HF_TEMPLATE:-json}"
RUNS="${HF_RUNS:-20}"
WARMUP="${HF_WARMUP:-3}"

if [ ! -f "$INPUT_PATH" ]; then
  echo "Input file not found: $INPUT_PATH" >&2
  exit 1
fi

echo "Building release binary..."
cargo build --release >/dev/null

OUT_DIR="benchmarks/hyperfine"
mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_JSON="$OUT_DIR/bench_${STAMP}.json"

echo "Running hyperfine on $INPUT_PATH with budgets [$BUDGETS_STR] and template '$TEMPLATE'..."
hyperfine \
  --shell=none \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  --export-json "$OUT_JSON" \
  --parameter-list n "$BUDGETS_STR" \
  'target/release/headson --profile --input '"$INPUT_PATH"' -n {n} -f '"$TEMPLATE"''

echo "Saved results to: $OUT_JSON"
echo "Tip: set HF_INPUT=/path/to/large.json and rerun for production-sized data."
