Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output grep.gif

Require hson

Type "# Grep mode guarantees matches stay in view (even across globs + mixed file types)." Sleep 200ms Enter
Type "hson --glob 'fixtures/tree_demo/project/**/*' --grep compute -n 12" Sleep 400ms Enter
Sleep 5s

Type "# Grep-like output: use a tiny per-file budget so only matches + parents fit." Sleep 200ms Enter
Type "hson --glob 'fixtures/tree_demo/project/**/*' --grep compute -n 1" Sleep 400ms Enter
Sleep 5s

Type "# Minor feature: keep all files in view with --grep-show=all." Sleep 200ms Enter
Type "hson --glob 'fixtures/tree_demo/project/**/*' --grep compute --grep-show=all -n 12" Sleep 400ms Enter
Sleep 5s
