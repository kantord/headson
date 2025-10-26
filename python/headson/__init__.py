from __future__ import annotations

# Import the compiled extension function
from .headson import summarize as _native_summarize  # type: ignore

__all__ = ["summarize"]


def summarize(
    text: str,
    *,
    template: str = "pseudo",
    character_budget: int | None = None,
    sampling: str | None = None,
    tail: bool | None = None,
) -> str:
    # Determine effective sampling with backwards compatibility
    eff_sampling = sampling or ("tail" if (tail is True) else "balanced")
    try:
        # Prefer new API (sampling kw)
        return _native_summarize(
            text,
            template=template,
            character_budget=character_budget,
            sampling=eff_sampling,
        )
    except TypeError:
        # Fallback for older native builds that only accept `tail`
        return _native_summarize(
            text,
            template=template,
            character_budget=character_budget,
            tail=(eff_sampling == "tail"),
        )
