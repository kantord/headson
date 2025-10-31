from __future__ import annotations

from . import headson as _ext  # type: ignore


def _map_template(fmt: str, style: str) -> str:
    f = fmt.lower()
    s = style.lower()
    if f == "yaml" or f == "yml":
        return "yaml"
    if s == "strict":
        return "json"
    if s == "default":
        return "pseudo"
    if s == "detailed":
        return "js"
    return "pseudo"


def summarize(
    text: str,
    *,
    format: str = "auto",
    style: str = "default",
    input_format: str = "json",
    character_budget: int | None = None,
    skew: str = "balanced",
    template: str | None = None,
) -> str:
    """Summarize JSON/YAML input with budget-sensitive formatting.

    Accepts both the new (format/style) and legacy (template) keyword styles.
    """
    if template is not None and (format != "auto" or style != "default" or input_format != "json"):
        raise TypeError("use either template=... or format/style/input_format, not both")
    # If legacy template provided explicitly, call through directly.
    if template is not None:
        return _ext.summarize(
            text,
            template=template,
            character_budget=character_budget,
            skew=skew,
        )
    # Try new-style call first.
    try:
        return _ext.summarize(
            text,
            format=format,
            style=style,
            input_format=input_format,
            character_budget=character_budget,
            skew=skew,
        )
    except TypeError:
        # Fallback to legacy template-based binding installed in the environment.
        tmpl = _map_template(format, style)
        try:
            return _ext.summarize(
                text,
                template=tmpl,
                character_budget=character_budget,
                skew=skew,
            )
        except TypeError:
            # Older legacy builds may not accept `skew`; try without it.
            if character_budget is not None:
                return _ext.summarize(
                    text,
                    template=tmpl,
                    character_budget=character_budget,
                )
            return _ext.summarize(
                text,
                template=tmpl,
            )


__all__ = ["summarize"]
