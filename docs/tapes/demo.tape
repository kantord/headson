Set FontSize 26
Set Width 1560
Set Height 1400
Set TypingSpeed 25ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output demo.gif

Require curl
Require hson

Type "# Headson gives you structure-aware summaries for data" Sleep 200ms Enter
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson" Sleep 400ms Enter
Sleep 6s

Type "# You get a concise view of structure and content within your line or character limit." Sleep 200ms Enter
Type "# Fixed number of characters" Sleep 200ms Enter
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -c 100" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -c 500" Sleep 400ms Enter
Sleep 6s
Type "# Fixed number of lines" Sleep 200ms Enter
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -n 5" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -n 50" Sleep 400ms Enter
Sleep 6s
