import json
import pytest

import headson


def count_chars(s: str) -> int:
    # Count Unicode code points excluding a trailing newline the Rust CLI often adds.
    return s.rstrip("\n").__len__()


def test_char_budget_caps_output_length_for_multibyte():
    # Use multibyte characters to differentiate bytes vs chars.
    data = {"s": "é" * 200}
    text = json.dumps(data)
    out = headson.summarize(text, format="json", style="strict", char_budget=60)
    # Ensure output does not exceed the character budget when measured as code points.
    assert count_chars(out) <= 60


def test_char_and_byte_budget_conflict_raises():
    data = {"x": [1, 2, 3]}
    text = json.dumps(data)
    with pytest.raises(RuntimeError):
        headson.summarize(
            text,
            format="json",
            style="strict",
            char_budget=50,
            byte_budget=50,
        )
