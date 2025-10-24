try:
    # Import public API from the compiled extension submodule
    from .headson import summarize  # type: ignore
except Exception as _e:  # pragma: no cover
    # Fallback: leave module import to fail at call time with a clearer error
    summarize = None  # type: ignore

__all__ = ["summarize"]
