# Re-export the public API from the compiled extension submodule
from .headson import summarize  # type: ignore

__all__ = ["summarize"]
