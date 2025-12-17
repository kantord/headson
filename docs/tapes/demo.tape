Set FontSize 26
Set Width 1560
Set Height 1400
Set TypingSpeed 25ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output demo.gif

Require hson

Type "# Headson gives you structure-aware summaries for data" Sleep 200ms Enter
Type "hson page1.json" Sleep 400ms Enter
Sleep 6s

Type "# You get a concise view of structure and content within your byte, line, or character limit." Sleep 200ms Enter
Type "# Fixed number of bytes" Sleep 200ms Enter
Type "hson page1.json -c 100" Sleep 400ms Enter
Sleep 6s
Type "# Fixed number of characters" Sleep 200ms Enter
Type "hson page1.json -u 100" Sleep 400ms Enter
Sleep 6s
Type "# Fixed number of lines" Sleep 200ms Enter
Type "hson page1.json -n 5" Sleep 400ms Enter
Sleep 6s
Type "hson page1.json -n 50" Sleep 400ms Enter
Sleep 6s
Type "# It also understands source files." Sleep 200ms Enter
Type "hson code_sample.py -n 5" Sleep 400ms Enter
Sleep 6s
Type "# Mix languages and headson keeps each file balanced." Sleep 200ms Enter
Type "hson code_sample.py code_sample.ts -n 3" Sleep 400ms Enter
Sleep 6s
