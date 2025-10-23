# headson Python bindings

This package exposes the core `headson` JSON preview renderer to Python.

Example:

```python
import headson

print(headson.summarize_bytes(b'{"a": 1, "b": [1,2,3]}', template="pseudo"))
print(headson.summarize_files(["sample.json"], template="js", global_budget=10_000))
print(headson.summarize_texts([
    {"path": "a.json", "content": "{\"a\": 1}"},
    {"path": "b.json", "content": "{\"b\": [1,2,3]}"},
]))
```

Install for development:

```
pipx install maturin
maturin develop -m python/pyproject.toml
```

