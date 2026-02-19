#!/usr/bin/env bash
# 14 — Sorting and dates: asort, asorti, join, parsedate, strftime
#
# Story: sort data, work with dates, then combine both to build
# a timeline from real CSV data.
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. asort + join — sort and reassemble"

echo "Sort words alphabetically:"
show $FK '{ for (i=1;i<=NF;i++) a[i]=$i; asort(a); print join(a, " → ") }' "$TMPDIR/fruits.txt"

echo ""
echo "Sort numerically (highest revenue regions):"
show $FK -H '{
    rev[$region] += $revenue
}
END {
    for (r in rev)
        out[r] = sprintf("$%6d  %s", rev[r], r)
    asort(out)
    for (i in out) print "  " out[i]
}' "$TMPDIR/sales.csv"

section "2. asorti — sort by key"

echo "Alphabetical product index:"
show $FK -i csv -H '{
    products[$product] += $units
}
END {
    n = asorti(products, keys)
    for (i = 1; i <= n; i++)
        printf "  %-10s %d units\n", keys[i], products[keys[i]]
}' "$TMPDIR/sales.csv"

section "3. Date parsing and formatting"

echo "Parse date strings into structured output:"
show $FK -H '{
    ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
    dow = strftime("%A", ts)
    short = strftime("%b %d", ts)
    printf "  %-20s %-10s %-6s  %3d people\n", $event, dow, short, $attendees
}' "$TMPDIR/events.csv"

echo ""
echo "Date arithmetic — days until each event from a reference date:"
show $FK -H 'BEGIN { ref = mktime("2025 01 01 00 00 00") }
{
    ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
    days = int((ts - ref) / 86400)
    printf "  Day %3d  %s  (%s)\n", days, $event, strftime("%b %d", ts)
}' "$TMPDIR/events.csv"

section "4. Putting it together — sorted event timeline"

echo "Orders sorted chronologically with formatted dates:"
show $FK -H '{
    ts = parsedate($created_at, "%Y-%m-%d %H:%M:%S")
    line[NR] = sprintf("  %s  %-14s  $%8.2f  %s", strftime("%b %d", ts), trim($customer), $amount+0, $currency)
}
END {
    asort(line)
    for (i in line) print line[i]
}' "$TMPDIR/orders.csv"

printf "\n${C_BOLD}Done.${C_RESET} asort/asorti/join + parsedate/strftime — awk has none of these.\n"
