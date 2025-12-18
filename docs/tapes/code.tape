Set FontSize 26
Set Width 1560
Set Height 900
Set TypingSpeed 30ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output code.gif

Require hson

Type "# Source code: get an outline of the file without printing every line." Sleep 200ms Enter
Type "# Keeps important lines (like function/class headers) and omits less relevant blocks." Sleep 200ms Enter
Type "hson fixtures/code_demo/big.py -n 12" Sleep 400ms Enter
Sleep 5s

Type "# Works across languages; syntax highlighting + line numbers when colors are enabled." Sleep 200ms Enter
Type "hson fixtures/code_demo/big.ts -n 12" Sleep 400ms Enter
Sleep 5s
