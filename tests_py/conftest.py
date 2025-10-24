import os
import pathlib
import subprocess
import sys
import shutil


def _has_headson_with_summarize() -> bool:
    try:
        import headson  # type: ignore
        return hasattr(headson, "summarize")
    except Exception:
        return False


def pytest_sessionstart(session):  # noqa: D401
    """Build and install the extension before tests import it."""
    if os.environ.get("SKIP_RUST_BUILD") == "1":
        return

    if _has_headson_with_summarize():
        return

    repo_root = pathlib.Path(__file__).resolve().parents[1]
    pyproject = repo_root / "pyproject.toml"
    if not pyproject.exists():
        return

    use_maturin = (
        os.environ.get("USE_MATURIN_DEVELOP") == "1"
        or shutil.which("maturin") is not None
    )

    def build_with(cmd):
        print(f"[conftest] Building extension: {' '.join(cmd)}")
        subprocess.run(cmd, check=True)

    cmd = ["maturin", "develop", "--quiet"] if use_maturin else [
        sys.executable, "-m", "pip", "install", "-e", str(repo_root)
    ]
    if use_maturin and os.environ.get("RELEASE") == "1":
        cmd.append("--release")
    try:
        build_with(cmd)
    except FileNotFoundError:
        alt = [sys.executable, "-m", "pip", "install", "-e", str(repo_root)] if use_maturin else [
            "maturin", "develop", "--quiet"
        ]
        build_with(alt)

    try:
        import importlib
        importlib.invalidate_caches()
        sys.modules.pop('headson', None)
        try:
            import headson  # type: ignore
            print(f"[conftest] headson from: {getattr(headson, '__file__', '?')}")
        except Exception as e:
            print(f"[conftest] primary import failed: {e}; attempting direct extension import...")
            ext = importlib.import_module('headson.headson')
            sys.modules['headson'] = ext  # expose extension as package
            headson = ext  # type: ignore
            print(f"[conftest] using extension as package: {getattr(headson, '__file__', '?')}")

        if not hasattr(headson, "summarize"):
            if use_maturin:
                print("[conftest] summarize() missing; retry maturin develop...")
                build_with(["maturin", "develop", "--quiet"])
                importlib.invalidate_caches()
                sys.modules.pop('headson', None)
                try:
                    import headson as _headson  # type: ignore
                except Exception:
                    _headson = importlib.import_module('headson.headson')
                    sys.modules['headson'] = _headson
                print(f"[conftest] reloaded headson from: {getattr(_headson, '__file__', '?')}")
    except Exception as e:
        print(f"[conftest] Post-build import failed: {e}")
