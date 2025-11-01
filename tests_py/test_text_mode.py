import headson


def test_text_mode_basic_lines_default_style():
    text = "one\ntwo\nthree\n"
    out = headson.summarize(
        text,
        format="text",
        style="default",
        input_format="text",
        character_budget=100,
    )
    assert out.splitlines()[:3] == ["one", "two", "three"]
    # No quotes in text mode
    assert '"one"' not in out


def test_text_mode_omission_marker_under_budget():
    text = "\n".join(f"line{i}" for i in range(20)) + "\n"
    out = headson.summarize(
        text,
        format="text",
        style="default",
        input_format="text",
        character_budget=20,  # Force omission
    )
    assert "…\n" in out
