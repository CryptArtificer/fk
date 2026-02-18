#!/usr/bin/env bash
# 02 — Regex capture groups, gensub, string toolkit, bitwise
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. Regex capture groups — structured log parsing"
# awk match() has no capture groups. fk: match(s, re, arr).

show $FK '{
    match($0, "^([0-9.]+) .* \"([A-Z]+) ([^ ]+) .*\" ([0-9]+) ([0-9]+)", c)
    printf "  %-15s %-6s %-25s %s  %sB\n", c[1], c[2], c[3], c[4], c[5]
}' "$TMPDIR/access.log"

section "2. gensub — functional string replacement"
# awk: sub/gsub modify in place. fk: gensub returns a copy.

show_pipe "echo 'user=alice email=alice@example.com token=abc123secret' | $FK '{
    safe = gensub(\"token=[^ ]+\", \"token=***\", \"g\")
    safe = gensub(\"[a-zA-Z0-9.]+@[a-zA-Z0-9.]+\", \"***@***\", \"g\", safe)
    print \"  original:\", \$0
    print \"  redacted:\", safe
}'"

echo ""
echo "Replace only the 2nd occurrence:"
show_pipe "echo 'one-two-three-four-five' | $FK '{ print \"  \" gensub(\"-\", \"=DASH=\", 2) }'"

section "3. String toolkit — trim, reverse, chr, ord, hex"
# awk: none of these exist. You'd write 10-line functions for each.

show_pipe "printf '  hello world  \n  fk rocks  \n' | $FK '{ printf \"  trim: %-15s  rev: %s\n\", \"\\\"\" trim(\$0) \"\\\"\", reverse(trim(\$0)) }'"

echo ""
echo "ASCII table via chr():"
show_pipe "echo | $FK 'BEGIN { printf \"  \"; for (i=33; i<=126; i++) printf \"%s\", chr(i); print \"\" }'"

echo ""
echo "Encode/decode:"
show_pipe "echo 'Hello' | $FK '{ for (i=1; i<=length(\$0); i++) printf \"  %s → ord=%d hex=%s\n\", substr(\$0,i,1), ord(substr(\$0,i,1)), hex(ord(substr(\$0,i,1))) }'"

section "4. Bitwise operations + hex + zero-padded printf"
# awk: no bitwise ops, no hex output, no zero-padded printf.

echo "Unix permission decoder (zero-padded octal and hex):"
show_pipe "printf '511\n493\n420\n256\n' | $FK '{
    mode = \$1+0; u=and(rshift(mode,6),7); g=and(rshift(mode,3),7); o=and(mode,7)
    printf \"  %04d → %d%d%d  hex=0x%04x\n\", mode, u, g, o, mode
}'"

printf "\n${C_BOLD}Done.${C_RESET} fk's regex captures, gensub, and string builtins eliminate boilerplate.\n"
