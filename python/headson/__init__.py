from __future__ import annotations

from . import headson as _ext  # type: ignore


def summarize(
    text: str,
    *,
    format: str = "auto",
    style: str = "default",
    input_format: str = "json",
    character_budget: int | None = None,
    skew: str = "balanced",
) -> str:
    """Summarize JSON/YAML input with budget-sensitive formatting.

    Python API mirrors the CLI: select output `format` (auto|json|yaml),
    output `style` (strict|default|detailed), and `input_format` (json|yaml).
    """
    return _ext.summarize(
        text,
        format=format,
        style=style,
        input_format=input_format,
        character_budget=character_budget,
        skew=skew,
    )


__all__ = ["summarize"]
