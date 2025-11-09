Set FontSize 26
Set Width 1560
Set Height 1400
Set TypingSpeed 25ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output demo.gif

Require curl
Require hson

# Network JSON stream → default Pseudo style
Type "# Headson gives you structure-aware summaries for data" Sleep 200ms Enter
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -n10" Sleep 400ms Enter
Sleep 6s

