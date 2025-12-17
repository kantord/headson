Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output tree.gif

Require hson

Type "# Tree mode renders files inline with structure-aware previews." Sleep 200ms Enter
Type "hson --glob 'fixtures/tree_demo/project/**/*' --tree -n 10" Sleep 400ms Enter
Sleep 5s

Type "# Combine tree with grep to highlight matches inside the tree." Sleep 200ms Enter
Type "hson --glob 'fixtures/tree_demo/project/**/*' --tree --grep compute -n 12" Sleep 400ms Enter
Sleep 5s
