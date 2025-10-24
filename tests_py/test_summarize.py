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
    text = '{"arr": [' + ",".join(str(i) for i in range(100)) + "]}"
    out_small = headson.summarize(text, template=template, character_budget=40)
    out_large = headson.summarize(text, template=template, character_budget=400)
    assert len(out_small) <= len(out_large)


def test_summarize_budget_only_kw():
    text = '{"x": [1,2,3,4,5,6,7,8,9]}'
    out_10 = headson.summarize(text, template="json", character_budget=10)
    out_100 = headson.summarize(text, template="json", character_budget=100)
    assert len(out_10) <= len(out_100)


def test_pseudo_shows_ellipsis_on_truncation():
    text = '{"arr": [' + ",".join(str(i) for i in range(50)) + "]}"
    out = headson.summarize(text, template="pseudo", character_budget=30)
    assert "…" in out


def test_js_shows_comment_on_truncation():
    text = '{"arr": [' + ",".join(str(i) for i in range(50)) + "]}"
    out = headson.summarize(text, template="js", character_budget=30)
    assert "/*" in out and "more" in out


def test_exact_string_output_json_template():
    # Exact output check for simple string input
    text = '"hello"'
    out = headson.summarize(text, template="json", character_budget=100)
    assert out == '"hello"'
