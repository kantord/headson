#!/usr/bin/env bash
set -euo pipefail

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine not found. Install: https://github.com/sharkdp/hyperfine" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# Inputs (can be overridden via env or args). Default: live pipe generation.
ARG_PATH="${1:-}"
BUDGETS_STR="${HF_BUDGETS:-10,100,1000,10000}"
TEMPLATE="${HF_TEMPLATE:-json}"
RUNS="${HF_RUNS:-20}"
WARMUP="${HF_WARMUP:-3}"
GEN_COUNT="${HF_GEN_COUNT:-200000}"
GEN_SEED="${HF_GEN_SEED:-42}"

# If a path is provided (positional or HF_INPUT), use file mode; otherwise pipe mode.
USE_FILE=false
INPUT_PATH=""
if [ -n "${HF_INPUT:-}" ]; then
  USE_FILE=true
  INPUT_PATH="$HF_INPUT"
elif [ -n "$ARG_PATH" ]; then
  USE_FILE=true
  INPUT_PATH="$ARG_PATH"
fi

if [ "$USE_FILE" = true ] && [ ! -f "$INPUT_PATH" ]; then
  echo "Input file not found: $INPUT_PATH" >&2
  exit 1
fi

echo "Building release binary..."
cargo build --release >/dev/null

OUT_DIR="benchmarks/hyperfine"
mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_JSON="$OUT_DIR/bench_${STAMP}.json"

if [ "$USE_FILE" = false ]; then
  # Live generation: pipe the example generator into headson (generator + headson end-to-end).
  echo "Building example generator..."
  cargo build --release --examples >/dev/null
  echo "Running hyperfine (pipe mode) with $GEN_COUNT items, seed=$GEN_SEED, budgets [$BUDGETS_STR], template '$TEMPLATE'..."
  # Note: we cannot use --shell=none for pipelines; shell startup overhead is negligible for large runs.
  hyperfine \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --export-json "$OUT_JSON" \
    --parameter-list n "$BUDGETS_STR" \
    "target/release/examples/genfixture --count $GEN_COUNT --seed $GEN_SEED | target/release/headson --profile -n {n} -f $TEMPLATE > /dev/null"
else
  echo "Running hyperfine on $INPUT_PATH with budgets [$BUDGETS_STR] and template '$TEMPLATE'..."
  hyperfine \
    --shell=none \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --export-json "$OUT_JSON" \
    --parameter-list n "$BUDGETS_STR" \
    'target/release/headson --profile --input '"$INPUT_PATH"' -n {n} -f '"$TEMPLATE"''
fi

echo "Saved results to: $OUT_JSON"
echo "Tip: set HF_INPUT=/path/to/large.json and rerun for production-sized data."
