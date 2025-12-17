Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output sort.gif

Require hson
Require git

Type "# Filesets are sorted by default." Sleep 200ms Enter
Type "# Order: frecency (git history) with mtime fallback." Sleep 200ms Enter
Type "# Use --no-sort to keep input order and skip the repo scan." Sleep 200ms Enter
Type "cd ../.." Sleep 200ms Enter
Sleep 300ms

Type "# Repo snapshot in a fixed total number of lines (global budget)." Sleep 200ms Enter
Type "hson --glob 'src/**/*.rs' --glob 'python/src/**/*.rs' --glob 'README.md' --glob 'Cargo.toml' --tree -n 2 --global-lines 22" Sleep 400ms Enter
Sleep 5s

Type "# Same idea, but cap total size in bytes (global budget)." Sleep 200ms Enter
Type "hson --glob 'src/**/*.rs' --glob 'python/src/**/*.rs' --glob 'README.md' --glob 'Cargo.toml' --tree -c 220 -C 900" Sleep 400ms Enter
Sleep 5s

Type "# Deterministic ordering: keep the glob/arg order." Sleep 200ms Enter
Type "hson --glob 'src/**/*.rs' --glob 'python/src/**/*.rs' --glob 'README.md' --glob 'Cargo.toml' --tree -n 2 --global-lines 22 --no-sort" Sleep 400ms Enter
Sleep 5s
