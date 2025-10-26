from __future__ import annotations

# Directly re-export the compiled extension function with the final signature.
from .headson import summarize  # type: ignore

__all__ = ["summarize"]
