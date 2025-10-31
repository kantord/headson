import os
import pathlib
import subprocess
import sys
import shutil
import importlib
import pytest


def _has_headson_with_summarize() -> bool:
    try:
        import headson  # type: ignore

        if not hasattr(headson, "summarize"):
            return False
        # Probe the compiled extension directly for new-style kwargs support.
        try:
            ext = importlib.import_module("headson.headson")
        except Exception:
            return False
        try:
            _ = ext.summarize("{}", format="json", style="strict", character_budget=1)
        except TypeError:
            return False
        except Exception:
            return True
        return True
    except Exception:
        return False


def pytest_sessionstart(session):  # noqa: D401
    """Build and install the extension before tests import it."""
    if os.environ.get("SKIP_RUST_BUILD") == "1":
        return

    # Always (re)build to avoid stale binary signature mismatches.

    repo_root = pathlib.Path(__file__).resolve().parents[1]
    # Ensure local Python package (wrapper) is importable ahead of any globally installed extension.
    pkg_dir = repo_root / "python"
    if str(pkg_dir) not in sys.path:
        sys.path.insert(0, str(pkg_dir))

    # Remove previously built extension artifacts to avoid stale native modules.
    try:
        so_glob = list((pkg_dir / "headson").glob("headson*.so"))
        for p in so_glob:
            try:
                p.unlink()
            except Exception:
                pass
        # Also clear cargo target for the python crate to force rebuild
        py_target = pkg_dir / "target"
        if py_target.exists():
            import shutil as _shutil

            _shutil.rmtree(py_target, ignore_errors=True)
    except Exception:
        pass
    pyproject = repo_root / "pyproject.toml"
    if not pyproject.exists():
        return

    use_maturin = (
        os.environ.get("USE_MATURIN_DEVELOP") == "1" or shutil.which("maturin") is not None
    )

    def build_with(cmd):
        print(f"[conftest] Building extension: {' '.join(cmd)}")
        subprocess.run(cmd, check=True)

    # Proactively remove any previously installed `headson` to avoid stale binaries.
    try:
        subprocess.run([sys.executable, "-m", "pip", "uninstall", "-y", "headson"], check=False)
    except Exception:
        pass

    cmd = (
        ["maturin", "develop", "--quiet"]
        if use_maturin
        else [sys.executable, "-m", "pip", "install", "-e", str(repo_root / "python")]
    )
    if use_maturin and os.environ.get("RELEASE") == "1":
        cmd.append("--release")
    try:
        build_with(cmd)
    except FileNotFoundError:
        alt = (
            [sys.executable, "-m", "pip", "install", "-e", str(repo_root)]
            if use_maturin
            else ["maturin", "develop", "--quiet"]
        )
        build_with(alt)

    try:
        import importlib

        importlib.invalidate_caches()
        # Drop both the package and extension submodule from sys.modules
        sys.modules.pop("headson", None)
        sys.modules.pop("headson.headson", None)

        import headson  # type: ignore

        print(f"[conftest] headson from: {getattr(headson, '__file__', '?')}")
        # Ensure extension submodule is the freshly built one and supports new kwargs
        ext = importlib.import_module("headson.headson")
        print(f"[conftest] headson.headson from: {getattr(ext, '__file__', '?')}")
        try:
            _ = ext.summarize("{}", format="json", style="strict", character_budget=1)
        except TypeError as e:
            msg = (
                f"headson extension lacks new kwargs (format/style): {e}\n"
                "Tests require a fresh build. Try: pip uninstall -y headson &&\n"
                "maturin develop --quiet"
            )
            pytest.exit(msg, returncode=1)
    except Exception as e:
        pytest.exit(f"[conftest] Post-build import failed: {e}", returncode=1)
