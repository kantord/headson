# headson Python bindings

Minimal Python API for the `headson` preview renderer.

API

- `headson.summarize(text: str, *, format: str = "auto", style: str = "default", input_format: str = "json", character_budget: int | None = None, skew: str = "balanced") -> str`
  - `format`: output format — `"auto" | "json" | "yaml"`.
  - `style`: output style — `"strict" | "default" | "detailed"`.
  - `input_format`: ingestion format — `"json" | "yaml"`.
  - `character_budget`: maximum output size in characters (defaults to 500 if not set).
  - `skew`: one of `"balanced" | "head" | "tail"`.
    - `balanced` (default), `head` keeps first N, `tail` keeps last N. Display styles place omission markers accordingly; strict JSON remains unannotated.
  - Notes:
    - For single inputs, `format="auto"` maps to the JSON family; set `format="yaml"` to emit YAML.

Examples:

```python
import headson

# Human-friendly JSON (Pseudo) with a small budget
print(headson.summarize('{"a": 1, "b": [1,2,3]}', format="json", style="default", character_budget=80))

# Strict JSON stays valid JSON
print(headson.summarize('{"a": 1, "b": {"c": 2}}', format="json", style="strict", character_budget=10_000))

# Annotated JSON (JS) with tail skew: prefer the end of arrays when truncating
arr = ','.join(str(i) for i in range(100))
print(headson.summarize('{"arr": [' + arr + ']}', format="json", style="detailed", character_budget=60, skew="tail"))

# YAML styles: strict (no comments), default (… comments), detailed (counts)
doc = 'root:\n  items: [1,2,3,4,5,6,7,8,9,10]\n'
print(headson.summarize(doc, format="yaml", style="strict", input_format="yaml", character_budget=60))
print(headson.summarize(doc, format="yaml", style="default", input_format="yaml", character_budget=60))
print(headson.summarize(doc, format="yaml", style="detailed", input_format="yaml", character_budget=60))

# Note: tail mode affects only display styles; strict JSON stays strict.
```

Install for development:

```
pipx install maturin
# Option A: maturin directly
maturin develop -m pyproject.toml

# Option B: uv (recommended for dev)
uv add --dev maturin pytest
uv sync
uv run --no-sync maturin develop -r
uv run --no-sync pytest -q
```
