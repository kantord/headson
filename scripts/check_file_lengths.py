#!/usr/bin/env python3
"""
Fail if any Rust/Python file exceeds a configurable line cap.

Env overrides:
  HEADSON_MAX_RS_LINES (default: 2200)
  HEADSON_MAX_PY_LINES (default: 1200)
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    cap = int(os.environ.get("HEADSON_MAX_LINES", "1942"))
    failures: list[str] = []

    for arg in argv:
        path = Path(arg)
        if not path.is_file():
            continue
        suffix = path.suffix.lower()
        if suffix not in {".rs", ".py"}:
            continue
        try:
            with path.open("r", encoding="utf-8", errors="ignore") as fh:
                lines = sum(1 for _ in fh)
        except OSError as exc:
            failures.append(f"{path}: unable to read file ({exc})")
            continue
        if lines > cap:
            failures.append(f"{path}: {lines} lines (limit {cap})")

    if failures:
        sys.stderr.write(
            "File length check failed:\n" + "\n".join(f"- {f}" for f in failures) + "\n"
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
