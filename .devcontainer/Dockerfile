# Rust (stable, matches CI) + uv, on the standard devcontainer base (non-root
# `vscode` user, uid 1000 — required for enwiro's containerized-launch path,
# which maps the host uid onto this image's own passwd entry via
# `--userns=keep-id`).
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# uv manages its own Python installs (the project pins 3.14 via
# .python-version), so no separate Python feature/toolchain is needed here.
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

# maturin uses patchelf to set the extension module's rpath; without it,
# `maturin develop` still succeeds but only with a warning on every run.
RUN apt-get update && apt-get install -y --no-install-recommends patchelf \
    && rm -rf /var/lib/apt/lists/*
