Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output code.gif

Require hson

Type "# Source code mode: indentation-aware, line-atomic summaries." Sleep 200ms Enter
Type "# Headson builds a heuristic block tree and keeps the most informative lines." Sleep 200ms Enter
Type "hson fixtures/tree_demo/project/src/main.py -n 14" Sleep 400ms Enter
Sleep 5s

Type "# Syntax highlighting + line numbers (colors auto-enable with CLICOLOR_FORCE=1)." Sleep 200ms Enter
Type "hson fixtures/tree_demo/project/src/lib/util.ts -n 14" Sleep 400ms Enter
Sleep 5s
