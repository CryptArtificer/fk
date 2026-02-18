#!/usr/bin/env bash
# 04 — Sorting (asort, asorti, join) and date parsing
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. asort — sort array values"
# awk: no built-in sort. fk: asort (by value), asorti (by key), join.

echo "Sort values alphabetically:"
show $FK '{ a[NR] = $0 } END { asort(a); print join(a, "\n") }' "$TMPDIR/fruits.txt"

section "2. Sort CSV by revenue (descending)"

show $FK -H '{
    rev[NR] = $revenue + 0
    line[NR] = sprintf("%-8s %-8s $%s", $region, $product, $revenue)
}
END {
    for (i = 1; i <= NR; i++) order[i] = i
    for (i = 1; i <= NR; i++)
        for (j = i+1; j <= NR; j++)
            if (rev[order[i]] < rev[order[j]]) {
                tmp = order[i]; order[i] = order[j]; order[j] = tmp
            }
    for (i = 1; i <= NR; i++) printf "  %s\n", line[order[i]]
}' "$TMPDIR/sales.csv"

section "3. Sort keys + join into a single line"

show_pipe "echo 'cherry apple banana date' | $FK '{ for (i=1;i<=NF;i++) a[i]=\$i; asort(a); print \"  \" join(a, \" → \") }'"

section "4. Date parsing + formatting"
# POSIX awk: no date functions at all. fk: parsedate, strftime, mktime.

show $FK -H '{
    ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
    dow = strftime("%A", ts)
    short = strftime("%b %d", ts)
    printf "  %-20s %-10s %-6s  %d people\n", $event, dow, short, $attendees
}' "$TMPDIR/events.csv"

printf "\n${C_BOLD}Done.${C_RESET} asort/asorti/join + parsedate/strftime — awk has none of these.\n"
