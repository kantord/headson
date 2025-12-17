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
Type "cd fixtures/sort_demo" Sleep 200ms Enter
Sleep 300ms

Type "# Make one file 'recent' so it bubbles to the top via mtime fallback." Sleep 200ms Enter
Type "touch -t 200001010000 data/alpha.json data/zeta.json src/main.py src/util.ts" Sleep 400ms Enter
Type "sleep 1 && touch data/zeta.json" Sleep 400ms Enter
Sleep 5s

Type "# Whole folder snapshot in a fixed total number of lines (global budget)." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -n 1 --global-lines 16" Sleep 400ms Enter
Sleep 5s

Type "# Deterministic ordering: keep the glob/arg order." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -n 1 --global-lines 16 --no-sort" Sleep 400ms Enter
Sleep 5s

Type "# Same idea, but cap total size in bytes (global budget)." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -c 120 -C 700" Sleep 400ms Enter
Sleep 5s
