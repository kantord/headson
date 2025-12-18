Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output sort.gif

Require hson
Require git

Type "# In multi-file mode, headson shows frequent+recent files first." Sleep 200ms Enter
Type "# Rarely touched files drift to the end (git history; mtime fallback)." Sleep 200ms Enter
Sleep 300ms

Type "cd $R" Sleep 300ms Enter
Sleep 2s

Type "# Whole folder snapshot with a strict total size cap (global bytes)." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -c 180 -C 260" Sleep 400ms Enter
Sleep 5s

Type "# Compare: preserve input order (no sorting)." Sleep 200ms Enter
Type "hson --glob 'src/*' --glob 'data/*' --tree -c 180 -C 260 --no-sort" Sleep 400ms Enter
Sleep 5s
