import textwrap

import headson


def _yaml_sample() -> str:
    return textwrap.dedent(
        """
        root:
          items: [1,2,3,4,5,6,7,8,9,10]
          obj:
            a: 1
            b: 2
            c: 3
            d: 4
        """
    ).strip()


def test_yaml_default_uses_ellipsis_comment():
    y = _yaml_sample()
    out = headson.summarize(
        y,
        format="yaml",
        style="default",
        input_format="yaml",
        character_budget=60,
    )
    assert "# …" in out, f"expected ellipsis comment in default style: {out!r}"


def test_yaml_strict_has_no_comments():
    y = _yaml_sample()
    out = headson.summarize(
        y,
        format="yaml",
        style="strict",
        input_format="yaml",
        character_budget=60,
    )
    assert "#" not in out, f"did not expect comments in strict YAML: {out!r}"


def test_yaml_detailed_shows_counts():
    y = _yaml_sample()
    out = headson.summarize(
        y,
        format="yaml",
        style="detailed",
        input_format="yaml",
        character_budget=60,
    )
    assert (
        "more items" in out or "more properties" in out
    ), f"expected numeric counts comment in detailed YAML: {out!r}"
