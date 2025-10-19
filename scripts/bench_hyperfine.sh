#!/usr/bin/env bash
set -euo pipefail

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine not found. Install: https://github.com/sharkdp/hyperfine" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# Inputs (can be overridden via env). No args needed; runs both pipe and file scenarios.
BUDGETS_STR="${HF_BUDGETS:-10,100,1000,10000}"
TEMPLATE="${HF_TEMPLATE:-json}"
RUNS="${HF_RUNS:-20}"
WARMUP="${HF_WARMUP:-3}"
GEN_COUNT="${HF_GEN_COUNT:-200000}"
GEN_SEED="${HF_GEN_SEED:-42}"
EX_FILE="examples/bench_fixture.json"

echo "Building release binary..."
cargo build --release >/dev/null

OUT_DIR="benchmarks/hyperfine"
mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_PIPE_JSON="$OUT_DIR/bench_pipe_${STAMP}.json"
OUT_FILE_JSON="$OUT_DIR/bench_file_${STAMP}.json"
OUT_GEN_JSON="$OUT_DIR/bench_gen_${STAMP}.json"
OUT_WRITE_JSON="$OUT_DIR/bench_write_${STAMP}.json"

# 1) Pipe scenario: generator → headson
echo "Building example generator..."
cargo build --release --examples >/dev/null
echo "Running hyperfine (PIPE) with $GEN_COUNT items, seed=$GEN_SEED, budgets [$BUDGETS_STR], template '$TEMPLATE'..."
hyperfine \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  --export-json "$OUT_PIPE_JSON" \
  --parameter-list n "$BUDGETS_STR" \
  "target/release/examples/genfixture --count $GEN_COUNT --seed $GEN_SEED | target/release/headson --profile -n {n} -f $TEMPLATE > /dev/null"

# 2) File scenario: generator → file → headson --input file
echo "Generating file fixture ($GEN_COUNT items) at $EX_FILE ..."
mkdir -p "examples"
target/release/examples/genfixture --count "$GEN_COUNT" --seed "$GEN_SEED" > "$EX_FILE"

BYTES=$(wc -c < "$EX_FILE" | tr -d ' ')
if command -v du >/dev/null 2>&1; then
  HR=$(du -h "$EX_FILE" | cut -f1)
else
  HR="${BYTES}B"
fi
echo "Fixture size: $BYTES bytes ($HR)"

echo "Running hyperfine (FILE) with budgets [$BUDGETS_STR], template '$TEMPLATE'..."
hyperfine \
  --shell=none \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  --export-json "$OUT_FILE_JSON" \
  --parameter-list n "$BUDGETS_STR" \
  'target/release/headson --profile --input '"$EX_FILE"' -n {n} -f '"$TEMPLATE"''

# 3) Generator-only and write-only micro-benches
echo "Running hyperfine (GEN → /dev/null)..."
hyperfine \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  --export-json "$OUT_GEN_JSON" \
  "target/release/examples/genfixture --count $GEN_COUNT --seed $GEN_SEED > /dev/null"

TMP_WRITE="examples/bench_write_tmp.json"
echo "Running hyperfine (GEN → file: $TMP_WRITE)..."
hyperfine \
  --prepare "rm -f $TMP_WRITE" \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  --export-json "$OUT_WRITE_JSON" \
  "target/release/examples/genfixture --count $GEN_COUNT --seed $GEN_SEED > $TMP_WRITE"

echo "Saved results to:"
echo "  PIPE:  $OUT_PIPE_JSON"
echo "  FILE:  $OUT_FILE_JSON"
echo "  GEN:   $OUT_GEN_JSON"
echo "  WRITE: $OUT_WRITE_JSON"

# Optional summary if jq is available
if command -v jq >/dev/null 2>&1; then
  echo
  echo "Summary (mean ms): budget  pipe  file  delta(pipe-file)"
  declare -A PIPE_MEAN FILE_MEAN
  while IFS=$'\t' read -r n ms; do PIPE_MEAN[$n]="$ms"; done < <(jq -r '.results[] | select(.parameters.n) | [.parameters.n, (.mean*1000)] | @tsv' "$OUT_PIPE_JSON")
  while IFS=$'\t' read -r n ms; do FILE_MEAN[$n]="$ms"; done < <(jq -r '.results[] | select(.parameters.n) | [.parameters.n, (.mean*1000)] | @tsv' "$OUT_FILE_JSON")
  for n in $(echo "$BUDGETS_STR" | tr ',' ' '); do
    p=${PIPE_MEAN[$n]:-}
    f=${FILE_MEAN[$n]:-}
    if [ -n "$p" ] && [ -n "$f" ]; then
      dp=$(awk -v a="$p" -v b="$f" 'BEGIN{ printf "%.1f", a-b }')
      printf "  %6s  %6.1f  %6.1f  %6s\n" "$n" "$p" "$f" "$dp"
    fi
  done
  echo
  if command -v numfmt >/dev/null 2>&1; then
    GEN_MS=$(jq -r '.results[0].mean*1000' "$OUT_GEN_JSON")
    WR_MS=$(jq -r '.results[0].mean*1000' "$OUT_WRITE_JSON")
  else
    GEN_MS=$(jq -r '.results[0].mean*1000' "$OUT_GEN_JSON")
    WR_MS=$(jq -r '.results[0].mean*1000' "$OUT_WRITE_JSON")
  fi
  echo "Generator-only mean: ${GEN_MS%.*} ms"
  echo "Generator→file mean: ${WR_MS%.*} ms"
else
  echo
  echo "Install jq to see a quick summary."
fi
