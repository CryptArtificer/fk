#!/usr/bin/env bash
# 21-phase20.sh — Array convenience & language constructs
#
# Run: ./examples/21-phase20.sh
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. collect() — build arrays without boilerplate"
# In standard awk you'd write { a[NR]=$col+0 } and manually skip blanks.
# collect(a, expr) auto-keys, skips NaN/empty, and returns the new count.

echo "Gather all revenues into an array, then summarise:"
show $FK -F, -H '{ collect(a, $revenue) }
END { printf "  n=%d sum=%d mean=%.0f median=%.0f\n",
      length(a), sum(a), mean(a), median(a) }' "$TMPDIR/sales.csv"
echo ""

echo "collect() pairs naturally with the stats builtins — no manual NR index,"
echo "no +0 coercion, no empty-string guards."

section "2. Array transforms — top, bottom, runtotal, norm"
# All four mutate the array in place and return a chainable value
# (count for top/bottom, array name for runtotal/norm).

echo "top(a, 3) — keep only the 3 largest values:"
show $FK -F, -H '{ collect(a, $revenue) }
END { top(a, 3); print "  ", join(a, ", ") }' "$TMPDIR/sales.csv"
echo ""

echo "bottom(a, 3) — keep only the 3 smallest:"
show $FK -F, -H '{ collect(a, $revenue) }
END { bottom(a, 3); print "  ", join(a, ", ") }' "$TMPDIR/sales.csv"
echo ""

echo "runtotal(a) — replace each value with its running total:"
show $FK -F, -H '{ collect(a, $revenue) }
END { runtotal(a); print "  ", join(a, ", ") }' "$TMPDIR/sales.csv"
echo ""

echo "norm(a) — scale values to the 0..1 range (min→0, max→1):"
show $FK -F, -H '{ collect(a, $revenue) }
END { norm(a)
  for (i = 1; i <= length(a); i++)
    printf "  %.2f\n", a[i]
}' "$TMPDIR/sales.csv"

section "3. window() — sliding window for streaming data"
# window(a, n, expr) maintains the last n values in array a,
# so you can compute moving statistics as records arrive.

echo "4-point moving average over latency data:"
show $FK '{ window(w, 4, $1)
  printf "line %2d: val=%4s  4-pt avg=%6.1f\n", NR, $1, mean(w)
}' "$TMPDIR/latencies.txt"

section "4. Record selection — every N, last N"
# Two new pattern types that reduce boilerplate for common sampling tasks.

echo "every 5 — fire on every 5th record (sampling, progress):"
show $FK 'every 5 { print "record", NR, "->", $0 }' "$TMPDIR/latencies.txt"
echo ""

echo "last 3 — process only the final 3 records (tail with action):"
show $FK 'last 3 { print $0 }' "$TMPDIR/latencies.txt"
echo ""

echo "last N works alongside normal rules — here we collect everything"
echo "but also sum the last 5:"
show $FK '{ collect(a, $1) } last 5 { sum += $1 }
END { print "  sum(last 5):", sum, " mean(all):", mean(a) }' "$TMPDIR/latencies.txt"

section "5. Sorted for-in — deterministic iteration order"
# Standard awk's for-in order is undefined. fk adds @sort modifiers
# so output is reproducible without piping through sort(1).

echo "@sort — iterate keys in ascending alphabetical order:"
show $FK -F, -H '{ rev[$region] += $revenue }
END { for (k in rev) @sort print "  ", k, rev[k] }' "$TMPDIR/sales.csv"
echo ""

echo "@rval — iterate by descending value (biggest first):"
show $FK -F, -H '{ rev[$region] += $revenue }
END { for (k in rev) @rval print "  ", k, rev[k] }' "$TMPDIR/sales.csv"

section "6. Composability — chaining it all together"
# These features compose: collect gathers, top/norm/runtotal transform,
# hist/plotbox visualise, join serialises.

echo "collect → top 10 → histogram → plotbox:"
show $FK '{ collect(a, $1) }
END { top(a, 10); print plotbox(hist(a), 30) }' "$TMPDIR/latencies.txt"
echo ""

echo "collect → norm → join (inline normalised values):"
show $FK -F, -H '{ collect(a, $units) }
END { norm(a); print "  normalized:", join(a, ", ") }' "$TMPDIR/sales.csv"
echo ""

echo "collect → runtotal → plotbox (cumulative revenue chart):"
show $FK -F, -H '{ collect(a, $revenue) }
END { runtotal(a); print plotbox(hist(a), 30) }' "$TMPDIR/sales.csv"

printf "\n${C_BOLD}Done.${C_RESET} 6 new builtins + 2 patterns + sorted for-in.\n"
