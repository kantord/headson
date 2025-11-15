from __future__ import annotations

# Re-export the compiled extension API directly.
from .headson import summarize  # type: ignore

__all__ = ["summarize"]
