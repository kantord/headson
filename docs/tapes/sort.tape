Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"
Env GIT_TERMINAL_PROMPT "0"

Output sort.gif

Require hson
Require git

Type "# Filesets are sorted by default." Sleep 200ms Enter
Type "# Order: frecency (git history) with mtime fallback." Sleep 200ms Enter
Type "# Use --no-sort to keep input order and skip the repo scan." Sleep 200ms Enter
Type "# This demo creates a tiny temporary git repo (no mtime tricks)." Sleep 200ms Enter
Sleep 300ms

Type "tmp=$(mktemp -d) && cd \"$tmp\" && git init -q" Sleep 400ms Enter
Type "git config user.email demo@example.com && git config user.name demo" Sleep 400ms Enter
Type "git config commit.gpgsign false" Sleep 300ms Enter
Type "mkdir -p data src" Sleep 300ms Enter
Type "printf '%s\n' '{\"alpha\":{\"items\":[1,2,3]}}' > data/alpha.json" Sleep 300ms Enter
Type "printf '%s\n' '{\"zeta\":{\"pipeline\":{\"compute\":{\"steps\":[\"load\",\"compute\",\"save\"]}}}}' > data/zeta.json" Sleep 300ms Enter
Type "printf '%s\n' 'def main():' '    print(\"hello\")' > src/main.py" Sleep 300ms Enter
Type "printf '%s\n' 'export function add(a: number, b: number) { return a + b; }' > src/util.ts" Sleep 300ms Enter
Type "git add . && git commit -qm 'init'" Sleep 500ms Enter
Type "printf '%s\n' '{\"zeta\":{\"pipeline\":{\"compute\":{\"steps\":[\"load\",\"compute\",\"save\"],\"runs\":1}}}}' > data/zeta.json" Sleep 300ms Enter
Type "git add data/zeta.json && git commit -qm 'touch zeta'" Sleep 500ms Enter
Type "printf '%s\n' '{\"zeta\":{\"pipeline\":{\"compute\":{\"steps\":[\"load\",\"compute\",\"save\"],\"runs\":2}}}}' > data/zeta.json" Sleep 300ms Enter
Type "git add data/zeta.json && git commit -qm 'touch zeta again'" Sleep 500ms Enter
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
