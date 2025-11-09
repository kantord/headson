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
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson" Sleep 400ms Enter
Sleep 6s

# Tight and larger budgets (line-based)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -n 8" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -n 20" Sleep 400ms Enter
Sleep 6s

# Styles: detailed (JS with comments) and strict JSON
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson --format json --template detailed" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson --format json --template strict" Sleep 400ms Enter
Sleep 6s

# Multiple files with a global budget (paginate via offset)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=80&offset=0' > page1.json" Sleep 400ms Enter
Sleep 1s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=80&offset=80' > page2.json" Sleep 400ms Enter
Sleep 1s
Type "hson --global-lines 24 --format json --template default -- page1.json page2.json" Sleep 400ms Enter
Sleep 6s

# YAML sample with detailed comments
Type "printf 'root:\n  items: [1,2,3,4,5,6,7,8,9,10]\n' > tmp.yaml" Sleep 200ms Enter
Sleep 500ms
Type "hson -n 6 --format yaml --template detailed --input-format yaml tmp.yaml" Sleep 400ms Enter
Sleep 6s

# Compact strict JSON (machine-friendly)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson -c 120 --format json --template strict --compact" Sleep 400ms Enter
Sleep 6s

# Head vs Tail sampling on the results array (at the end)
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson --head -c 120 --format json --template default" Sleep 400ms Enter
Sleep 6s
Type "curl -sS 'https://pokeapi.co/api/v2/pokemon?limit=151' | hson --tail -c 120 --format json --template detailed" Sleep 400ms Enter
Sleep 6s

Type "# Download two public-domain books (text/plain)" Sleep 200ms Enter
Type "rm -f book*.txt" Sleep 200ms Enter
Type "curl -L -sS https://www.gutenberg.org/cache/epub/77164/pg77164.txt -o book1.txt" Sleep 400ms Enter
Sleep 2s
Type "curl -L -sS https://www.gutenberg.org/cache/epub/2701/pg2701.txt -o book2.txt" Sleep 400ms Enter
Sleep 2s
Type "hson -n 40 book*.txt" Sleep 400ms Enter
Sleep 6s
Type "# Single-file text with styles (default vs detailed)" Sleep 200ms Enter
Type "hson -n 12 -t default book1.txt" Sleep 400ms Enter
Sleep 4s
Type "hson -n 12 -t detailed book1.txt" Sleep 400ms Enter
Sleep 4s
