Output demo.gif

Require curl
Require headson

Set Shell "bash"
Env CLICOLOR_FORCE "1"
Set FontSize 26
Set Width 1560
Set Height 1400
Set TypingSpeed 25ms

Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson" Sleep 400ms Enter
Sleep 6s

Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -n100" Sleep 400ms Enter
Sleep 6s


Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -n700" Sleep 400ms Enter
Sleep 6s


Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -f js" Sleep 400ms Enter
Sleep 6s


Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -f json" Sleep 400ms Enter
Sleep 6s
