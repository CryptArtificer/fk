#!/usr/bin/env bash
# 12 — Text processing: regex captures, gensub, string toolkit
#
# Story: you have web server logs and messy text. Parse, extract,
# transform, redact — all without external tools.
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. Regex capture groups — structured log parsing"

echo "Turn raw access-log lines into a clean table:"
show $FK '{
    match($0, "^([0-9.]+) .* \"([A-Z]+) ([^ ]+) .*\" ([0-9]+) ([0-9]+)", c)
    printf "  %-15s %-6s %-25s %s  %5sB\n", c[1], c[2], c[3], c[4], c[5]
}' "$TMPDIR/access.log"

echo ""
echo "Extract just the 5xx errors with the requesting IP:"
show $FK '{
    match($0, "^([0-9.]+) .* \" ([0-9]+) ", c)
    if (c[2]+0 >= 500) printf "  %s → %s\n", c[1], c[2]
}' "$TMPDIR/access.log"

section "2. gensub — functional string replacement"

echo "Redact sensitive fields without touching the original:"
show_pipe "echo 'user=alice email=alice@example.com token=abc123secret' | $FK '{
    safe = gensub(\"token=[^ ]+\", \"token=***\", \"g\")
    safe = gensub(\"[a-zA-Z0-9.]+@[a-zA-Z0-9.]+\", \"***@***\", \"g\", safe)
    print \"  original:\", \$0
    print \"  redacted:\", safe
}'"

echo ""
echo "Target a specific occurrence (replace only the 2nd dash):"
show_pipe "echo 'one-two-three-four-five' | $FK '{ print gensub(\"-\", \" | \", 2) }'"

section "3. String toolkit — trim, pad, reverse"

echo "Clean up messy whitespace and display aligned:"
show_pipe "printf '  hello world  \n  fk rocks  \n' | $FK '{
    t = trim(\$0)
    printf \"  %-20s → reversed: %s\n\", \"\\\"\" t \"\\\"\", reverse(t)
}'"

echo ""
echo "Pad and align columns from ragged input:"
show_pipe "printf 'Alice 95\nBob 87\nCarol 100\nDan 42\n' | $FK '{
    bar = repeat(\"█\", int(\$2/5))
    printf \"  %s %3d %s\n\", rpad(\$1, 6), \$2, bar
}'"

section "4. Character-level operations — chr, ord, hex"

echo "Build an ASCII table:"
show $FK 'BEGIN {
    for (i = 33; i <= 126; i++) {
        printf "  %3d  %4s  %s", i, hex(i), chr(i)
        if ((i - 32) % 6 == 0) print ""
    }
    print ""
}'

echo ""
echo "Encode a string to hex:"
show_pipe "echo 'Hello' | $FK '{ for (i=1; i<=length(\$0); i++) { c=substr(\$0,i,1); printf \"  %s → %d → %s\n\", c, ord(c), hex(ord(c)) } }'"

section "5. startswith / endswith — filter without regex"

show $FK -H '{
    if (startswith($"host-name", "web"))
        printf "  frontend: %s  cpu=%.1f%%\n", $"host-name", $"cpu-usage"
    else if (endswith($"host-name", "01"))
        printf "  primary:  %s  cpu=%.1f%%\n", $"host-name", $"cpu-usage"
}' "$TMPDIR/servers.csv"

printf "\n${C_BOLD}Done.${C_RESET} Regex captures, gensub, and 20+ string builtins — no boilerplate.\n"
