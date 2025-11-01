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


def test_text_mode_strict_truncates_without_marker():
    text = "\n".join(f"line{i}" for i in range(50)) + "\n"
    out = headson.summarize(
        text,
        format="text",
        style="strict",
        input_format="text",
        character_budget=30,
    )
    # No array-level omission summary in strict mode
    # (per-line string truncation may render a single '…' line, which is allowed)
    assert " more lines " not in out
    assert "line49\n" not in out


def test_text_mode_detailed_shows_count():
    text = "\n".join(f"line{i}" for i in range(50)) + "\n"
    out = headson.summarize(
        text,
        format="text",
        style="detailed",
        input_format="text",
        character_budget=30,
    )
    assert "…" in out and " more lines " in out


def test_text_mode_tail_places_marker_at_start():
    text = "\n".join(f"line{i}" for i in range(30)) + "\n"
    out = headson.summarize(
        text,
        format="text",
        style="default",
        input_format="text",
        skew="tail",
        character_budget=40,
    )
    first = out.splitlines()[0] if out else ""
    assert first == "…"
