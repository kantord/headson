#!/usr/bin/env bash

greet() {
  local name="$1"
  echo "Hello, ${name}"
}

compute() {
  local n="$1"
  local total=0
  for ((i=0; i<n; i++)); do
    if (( i % 2 == 0 )); then
      total=$((total + i))
    else
      total=$((total + i * 2))
    fi
  done
  echo "$total"
}

main() {
  greet "world"

  for ((i=0; i<3; i++)); do
    echo "i: $i"
  done

  value=$(compute 10)
  echo "value: ${value}"
}

main "$@"

