from __future__ import annotations

# Import the compiled extension function
from .headson import summarize as _native_summarize  # type: ignore

__all__ = ["summarize"]


def summarize(
    text: str,
    *,
    template: str = "pseudo",
    character_budget: int | None = None,
    skew: str = "balanced",
) -> str:
    # Route through the high-level skew option: "balanced" | "head" | "tail".
    return _native_summarize(
        text,
        template=template,
        character_budget=character_budget,
        skew=skew,
    )
