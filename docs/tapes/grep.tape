Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output grep.gif

Require hson

Type "# Grep mode guarantees matches stay in view." Sleep 200ms Enter
Type "hson code_sample.py --grep compute -n10" Sleep 400ms Enter
Sleep 5s

Type "# Grep across files; keep everything with --grep-show=all." Sleep 200ms Enter
Type "hson code_sample.py code_sample.ts --grep greet --grep-show=all -n8" Sleep 400ms Enter
Sleep 5s
