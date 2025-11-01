Set FontSize 26
Set Width 1560
Set Height 1400
Set TypingSpeed 25ms
Set Shell "bash"
Env CLICOLOR_FORCE "1"

Output demo.gif

Require curl
Require headson

# Network JSON stream → default Pseudo style
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson" Sleep 400ms Enter
Sleep 6s

# Tight and larger budgets
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -n100" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -n700" Sleep 400ms Enter
Sleep 6s

# Styles: detailed (JS with comments) and strict JSON
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -f json -t detailed" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -f json -t strict" Sleep 400ms Enter
Sleep 6s

# Multiple files with a global budget (paginate via offset)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=80&offset=0' > page1.json" Sleep 400ms Enter
Sleep 1s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=80&offset=80' > page2.json" Sleep 400ms Enter
Sleep 1s
Type "headson -N 400 -f json -t default page1.json page2.json" Sleep 400ms Enter
Sleep 6s

# YAML sample with detailed comments
Type "printf 'root:\n  items: [1,2,3,4,5,6,7,8,9,10]\n' > tmp.yaml" Sleep 200ms Enter
Sleep 500ms
Type "headson -n 60 -f yaml -t detailed -i yaml tmp.yaml" Sleep 400ms Enter
Sleep 6s

# Compact strict JSON (machine-friendly)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson -n 120 -f json -t strict --compact" Sleep 400ms Enter
Sleep 6s

# Head vs Tail sampling on the results array (at the end)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson --head -n 120 -f json -t default" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | headson --tail -n 120 -f json -t detailed" Sleep 400ms Enter
Sleep 6s
