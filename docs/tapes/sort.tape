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
Type "# The tape runner sets up a tiny git repo for this demo." Sleep 200ms Enter
Sleep 300ms

Type 'cd "$HEADSON_SORT_TAPE_REPO"' Sleep 300ms Enter
Sleep 2s

Type "# Git history (zeta.json changed repeatedly):" Sleep 200ms Enter
Type "git log --oneline -3 --name-only" Sleep 400ms Enter
Sleep 4s

Type "# Whole folder snapshot with a strict total line cap (global budget)." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -n 1 -H --global-lines 12" Sleep 400ms Enter
Sleep 5s

Type "# Deterministic ordering: keep the glob/arg order." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -n 1 -H --global-lines 12 --no-sort" Sleep 400ms Enter
Sleep 5s

Type "# Same idea, but cap total size in bytes (global budget)." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -c 120 -C 700" Sleep 400ms Enter
Sleep 5s
