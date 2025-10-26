# headson Python bindings

Minimal Python API for the `headson` JSON preview renderer.

Currently exported function:

- `headson.summarize(text: str, *, template: str = "pseudo", character_budget: int | None = None, tail: bool = False) -> str`
  - `template`: one of `"json" | "pseudo" | "js"`.
  - `character_budget`: maximum output size in characters (defaults to 500 if not set).
  - `tail`: prefer the end of arrays when truncating; strings are unaffected. Only affects display templates (`pseudo`/`js`); `json` remains strict JSON with no annotations.

Examples:

```python
import headson

# Pseudo template with a small budget (structure-aware preview)
print(headson.summarize('{"a": 1, "b": [1,2,3]}', template="pseudo", character_budget=80))

# Strict JSON template preserves valid JSON output
print(headson.summarize('{"a": 1, "b": {"c": 2}}', template="json", character_budget=10_000))

# JS template with tail preference: prefer the end of arrays when truncating
arr = ','.join(str(i) for i in range(100))
print(headson.summarize('{"arr": [' + arr + ']}', template="js", character_budget=60, tail=True))

# Note: tail mode affects only pseudo/js display templates; the json template stays strict.
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
