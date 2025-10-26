import json

import headson
import pytest


def test_summarize_json_roundtrip():
    text = '{"a": 1, "b": {"c": 2}}'
    out = headson.summarize(text, template="json", character_budget=10_000)
    # Must be valid JSON and contain original structure
    obj = json.loads(out)
    assert obj["a"] == 1
    assert obj["b"]["c"] == 2


@pytest.mark.parametrize("template", ["json", "pseudo", "js"])
def test_summarize_budget_affects_length(template):
    text = json.dumps({"arr": list(range(100))})
    out_small = headson.summarize(
        text,
        template=template,
        character_budget=40,
    )
    out_large = headson.summarize(
        text,
        template=template,
        character_budget=400,
    )
    assert len(out_small) <= len(out_large)


def test_summarize_budget_only_kw():
    text = json.dumps({"x": [1, 2, 3, 4, 5, 6, 7, 8, 9]})
    out_10 = headson.summarize(
        text,
        template="json",
        character_budget=10,
    )
    out_100 = headson.summarize(
        text,
        template="json",
        character_budget=100,
    )
    assert len(out_10) <= len(out_100)


def test_pseudo_shows_ellipsis_on_truncation():
    text = json.dumps({"arr": list(range(50))})
    out = headson.summarize(
        text,
        template="pseudo",
        character_budget=30,
    )
    assert "…" in out


def test_js_shows_comment_on_truncation():
    text = json.dumps({"arr": list(range(50))})
    out = headson.summarize(
        text,
        template="js",
        character_budget=30,
    )
    assert "/*" in out and "more" in out


def test_exact_string_output_json_template():
    # Exact output check for simple string input
    text = '"hello"'
    out = headson.summarize(
        text,
        template="json",
        character_budget=100,
    )
    assert out == '"hello"'


def test_tail_affects_arrays_pseudo():
    # Use a raw array to simplify assertions about leading markers.
    text = json.dumps(list(range(50)))
    out_tail = headson.summarize(
        text,
        template="pseudo",
        character_budget=30,
        sampling="tail",
    )
    out_head = headson.summarize(
        text,
        template="pseudo",
        character_budget=30,
        sampling="balanced",
    )
    assert out_tail != out_head
    # In tail mode (non-compact by default), the first non-empty line after '['
    # should be the omission marker.
    lines = out_tail.splitlines()
    # Find index of opener '[' line
    try:
        idx = next(i for i, line in enumerate(lines) if line.strip() == "[")
    except StopIteration:
        assert False, f"expected array opener, got: {out_tail!r}"
    # Next non-empty line should be ellipsis (may include trailing comma
    # when followed by items)
    following = next(
        (line.strip() for line in lines[idx + 1 :] if line.strip()),
        "",
    )
    assert following.startswith(
        "…"
    ), f"expected ellipsis after opener in tail mode, got: {out_tail!r}"


def test_tail_affects_arrays_js():
    text = json.dumps(list(range(50)))
    out_tail = headson.summarize(
        text,
        template="js",
        character_budget=30,
        sampling="tail",
    )
    out_head = headson.summarize(
        text,
        template="js",
        character_budget=30,
        sampling="balanced",
    )
    assert out_tail != out_head
    # Tail mode may render as multi-line (with '[' on its own line) or as a
    # single-line '[ /* N more items */ ]' if nothing else fits the budget.
    lines = out_tail.splitlines()
    try:
        idx = next(i for i, line in enumerate(lines) if line.strip() == "[")
        following = next(
            (line.strip() for line in lines[idx + 1 :] if line.strip()),
            "",
        )
        assert following.startswith(
            "/*"
        ), f"expected omission comment after opener in tail mode, got: {out_tail!r}"
    except StopIteration:
        # Single-line form like: '[ /* N more items */ ]'
        stripped = out_tail.strip()
        assert (
            stripped.startswith("[")
            and stripped.endswith("]")
            and "/*" in stripped
            and "*/" in stripped
        ), f"expected single-line omission comment inside brackets, got: {out_tail!r}"


def test_tail_json_remains_strict():
    text = json.dumps(list(range(50)))
    out = headson.summarize(
        text,
        template="json",
        character_budget=30,
        sampling="tail",
    )
    # Valid JSON and no visual omission markers.
    json.loads(out)
    assert "…" not in out and "/*" not in out


def test_head_affects_arrays_pseudo():
    text = json.dumps(list(range(50)))
    out_head = headson.summarize(
        text,
        template="pseudo",
        character_budget=30,
        sampling="head",
    )
    # Balanced may match head under tight budgets; check head’s placement instead of inequality.
    # In head mode (non-compact by default), the last non-empty line before ']' should
    # be the omission marker.
    lines = out_head.splitlines()
    # Find index of closer ']' line (last such line)
    try:
        idx = max(i for i, line in enumerate(lines) if line.strip() == "]")
    except ValueError:
        assert False, f"expected array closer, got: {out_head!r}"
    # Previous non-empty line should be ellipsis (may include trailing comma)
    preceding = next(
        (line.strip() for line in reversed(lines[:idx]) if line.strip()),
        "",
    )
    assert preceding.startswith(
        "…"
    ), f"expected ellipsis before closer in head mode, got: {out_head!r}"


def test_head_affects_arrays_js():
    text = json.dumps(list(range(50)))
    out_head = headson.summarize(
        text,
        template="js",
        character_budget=30,
        sampling="head",
    )
    # Balanced may coincide with head for small budgets; assert head’s placement explicitly.
    # Head mode may render as multi-line (']' on its own line) or as a
    # single-line '[ /* N more items */ ]' if nothing else fits.
    lines = out_head.splitlines()
    try:
        # Look for a line that is purely ']' and examine the previous line
        idx = next(i for i, line in enumerate(reversed(lines)) if line.strip() == "]")
        # idx is offset from the end
        closer_index = len(lines) - 1 - idx
        preceding = next(
            (line.strip() for line in reversed(lines[:closer_index]) if line.strip()),
            "",
        )
        assert preceding.startswith(
            "/*"
        ), f"expected omission comment before closer in head mode, got: {out_head!r}"
    except StopIteration:
        # Single-line form like: '[ /* N more items */ ]'
        stripped = out_head.strip()
        assert (
            stripped.startswith("[")
            and stripped.endswith("]")
            and "/*" in stripped
            and "*/" in stripped
        ), f"expected single-line omission comment inside brackets, got: {out_head!r}"


def test_head_json_remains_strict():
    text = json.dumps(list(range(50)))
    out = headson.summarize(
        text,
        template="json",
        character_budget=30,
        sampling="head",
    )
    # Valid JSON and no visual omission markers.
    json.loads(out)
    assert "…" not in out and "/*" not in out
