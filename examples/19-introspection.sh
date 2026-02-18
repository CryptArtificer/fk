#!/usr/bin/env bash
# 19 — Introspection: typeof, dump, timing, per-file hooks
#
# Story: you're debugging a data pipeline. These tools let you
# inspect values, trace execution, and measure performance —
# all from inside your fk program.
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. typeof — know what you're working with"

echo "Check types before operating on them:"
show $FK -H '{
    printf "  %-10s  revenue=%-12s type=%-14s  units=%-8s type=%s\n", $region, $revenue, typeof($revenue), $units, typeof($units)
}' "$TMPDIR/sales.csv"

echo ""
echo "Detect uninitialized values (useful for defensive coding):"
show $FK 'BEGIN {
    x = 42; split("a:b:c", arr, ":")
    printf "  assigned var:     %s\n", typeof(x)
    printf "  assigned element: %s\n", typeof(arr[1])
    printf "  missing var:      %s\n", typeof(ghost)
    printf "  missing element:  %s\n", typeof(arr[99])
    printf "  array itself:     %s\n", typeof(arr)
}'

section "2. dump — inspect values and arrays"

echo "Dump reveals the internal representation:"
show $FK 'BEGIN {
    x = 3.14
    dump(x)
    split("alice bob carol", team, " ")
    dump(team)
}'

echo ""
echo "Dump to a file — debug without cluttering output:"
show_pipe "echo 'hello world' | $FK '{ dump(\$0, \"$TMPDIR/debug.txt\"); print \"output:\", \$0 }'"
echo "  debug.txt contains:"
printf "  "; cat "$TMPDIR/debug.txt"; echo ""

section "3. BEGINFILE / ENDFILE — per-file processing"

printf "apple\nbanana\ncherry\n" > "$TMPDIR/fruit.txt"
printf "carrot\ncelery\nkale\npea\n" > "$TMPDIR/veg.txt"

echo "Wrap each file in a visual frame:"
show $FK '
BEGINFILE { printf "  ┌── %s ──\n", FILENAME }
          { printf "  │ %s\n", $0 }
ENDFILE   { printf "  └── %d lines ──\n\n", FNR }
' "$TMPDIR/fruit.txt" "$TMPDIR/veg.txt"

echo "Per-file stats with a grand total:"
show $FK -H '
BEGINFILE { file_sum = 0 }
          { file_sum += $revenue }
ENDFILE   { printf "  %-12s  total revenue: $%d\n", FILENAME, file_sum }
END       { printf "\n  All files:    total revenue: $%d\n", grand }
{ grand += $revenue }
' "$TMPDIR/sales.csv"

section "4. tic / toc — measure where time goes"

echo "Time a tight loop:"
show $FK 'BEGIN {
    tic()
    for (i = 0; i < 100000; i++) x += i
    printf "  100k additions: %.4f sec\n", toc()
}'

echo ""
echo "Named timers — profile different phases:"
show $FK 'BEGIN {
    tic("build")
    for (i = 0; i < 50000; i++) a[i] = i * 2
    printf "  build array: %.4f sec\n", toc("build")

    tic("scan")
    for (i = 0; i < 50000; i++) { if (a[i] > 99998) found++ }
    printf "  scan array:  %.4f sec  (found %d)\n", toc("scan"), found
}'

section "5. Putting it together — profiled multi-file analysis"

echo "Combine timing, per-file hooks, and stats in one program:"
seq 1 10000 > "$TMPDIR/numbers.txt"

show $FK -H '
BEGIN     { tic("total") }
BEGINFILE { tic(FILENAME) }
          {
              rev[$region] += $revenue
              units[$region] += $units
          }
ENDFILE   { printf "  %-12s  %4d rows  %.4f sec\n", FILENAME, FNR, toc(FILENAME) }
END       {
    elapsed = toc("total")
    print ""
    for (r in rev)
        printf "  %-6s  $%6d revenue  %4d units\n", r, rev[r], units[r]
    printf "\n  Processed %d records in %.4f sec\n", NR-1, elapsed
}' "$TMPDIR/sales.csv"

printf "\n${C_BOLD}Done.${C_RESET} typeof, dump, tic/toc, BEGINFILE/ENDFILE — debug and profile from inside fk.\n"
