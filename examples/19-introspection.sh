#!/usr/bin/env bash
# 19 — Introspection: dump, typeof, timing, BEGINFILE/ENDFILE
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. typeof — runtime type inspection"

show $FK 'BEGIN {
    x = 42
    y = "hello"
    split("a:b:c", arr, ":")
    printf "  typeof(x)       = %s\n", typeof(x)
    printf "  typeof(y)       = %s\n", typeof(y)
    printf "  typeof(arr)     = %s\n", typeof(arr)
    printf "  typeof(unknown) = %s\n", typeof(unknown)
    printf "  typeof(arr[1])  = %s\n", typeof(arr[1])
    printf "  typeof(arr[99]) = %s\n", typeof(arr[99])
}'

section "2. dump — inspect any value or array"

echo "Dump a scalar:"
show $FK 'BEGIN { x = 3.14; dump(x) }'

echo ""
echo "Dump an array:"
show $FK 'BEGIN {
    a["name"] = "Alice"
    a["age"]  = 30
    a["role"] = "engineer"
    dump(a)
}'

echo ""
echo "Dump from live data (first 3 records):"
show $FK -H 'NR <= 4 { dump($0) }' "$TMPDIR/sales.csv"

section "3. clk — wall-clock time"

show $FK 'BEGIN {
    printf "  Program started at clock = %.4f seconds\n", clk()
}'

section "4. tic / toc — stopwatch timers"

echo "Basic timing:"
show $FK 'BEGIN {
    tic()
    for (i = 0; i < 100000; i++) x += i
    printf "  100k iterations: %.4f seconds\n", toc()
}'

echo ""
echo "Named timers (run multiple in parallel):"
show $FK 'BEGIN {
    tic("setup")
    for (i = 0; i < 50000; i++) a[i] = i * 2
    printf "  setup: %.4f sec\n", toc("setup")

    tic("search")
    for (i = 0; i < 50000; i++) { if (a[i] > 99998) found++ }
    printf "  search: %.4f sec\n", toc("search")
}'

echo ""
echo "Per-file timing with BEGINFILE / ENDFILE:"
printf "1\n2\n3\n" > "$TMPDIR/small.txt"
seq 1 10000 > "$TMPDIR/big.txt"
show $FK '
BEGINFILE { tic(FILENAME) }
ENDFILE   { printf "  %-12s %6d records  %.4f sec\n", FILENAME, FNR, toc(FILENAME) }
{ sum += $1 }
END { printf "\n  total sum = %d\n", sum }
' "$TMPDIR/small.txt" "$TMPDIR/big.txt"

section "5. BEGINFILE / ENDFILE — per-file hooks"

echo "Print a banner around each file's output:"
printf "apple\nbanana\n" > "$TMPDIR/fruit.txt"
printf "carrot\ncelery\n" > "$TMPDIR/veg.txt"
show $FK '
BEGINFILE { printf "  ┌── %s ──\n", FILENAME }
          { printf "  │ %s\n", $0 }
ENDFILE   { printf "  └── %d records ──\n\n", FNR }
' "$TMPDIR/fruit.txt" "$TMPDIR/veg.txt"

section "6. dump to file — save diagnostics without cluttering output"

show_pipe "echo 'hello world' | $FK '{ dump(\$0, \"$TMPDIR/debug.txt\"); print \"processed:\", \$0 }'"
echo ""
echo "  Contents of debug.txt:"
printf "  "; cat "$TMPDIR/debug.txt"
echo ""

section "7. Putting it together — profiled CSV analysis"

show $FK -H '
BEGIN { tic("total") }
{
    rev[$region] += $revenue
    units[$region] += $units
}
END {
    elapsed = toc("total")
    for (r in rev)
        printf "  %-6s  revenue=$%6d  units=%d\n", r, rev[r], units[r]
    printf "\n  Processed %d records in %.4f sec (%.0f rec/sec)\n", NR-1, elapsed, (NR-1)/elapsed
}' "$TMPDIR/sales.csv"

printf "\n${C_BOLD}Done.${C_RESET} Introspection: typeof, dump, clk, tic/toc, BEGINFILE/ENDFILE.\n"
