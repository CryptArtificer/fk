#!/usr/bin/env bash
# 05 — Multi-stage pipelines: fk|fk, fk|awk, fk+sort, fk+xargs
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. fk | fk — multi-stage pipelines"

echo "Stage 1 (CSV parse + filter) → Stage 2 (aggregate):"
show_pipe "$FK -i csv -H '\$revenue+0 > 15000 { print \$region, \$product, \$revenue }' $TMPDIR/sales.csv | $FK '{ by[\$1] += \$3 } END { for (r in by) printf \"  %-6s \$%.0f\n\", r, by[r] }'"

echo ""
echo "Three-stage: generate → compute → filter:"
show_pipe "printf '1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n' | $FK '{ print \$1, \$1**2, \$1**3 }' | $FK '\$2 > 20 { printf \"  n=%-2d  n²=%-4d  n³=%d\n\", \$1, \$2, \$3 }'"

section "2. fk + sort + uniq — classic Unix pipeline, supercharged"

echo "Top request paths by count (fk extracts, sort+uniq counts, fk formats):"
show_pipe "$FK '{ match(\$0, \"\\\"[A-Z]+ ([^ ]+)\", c); print c[1] }' $TMPDIR/access.log | sort | uniq -c | sort -rn | $FK '{ printf \"  %3d  %s\n\", \$1, \$2 }'"

echo ""
echo "Unique IPs with request count:"
show_pipe "$FK '{ print \$1 }' $TMPDIR/access.log | sort | uniq -c | sort -rn | $FK '{ printf \"  %-15s %d requests\n\", \$2, \$1 }'"

section "3. fk + awk — interop both directions"

echo "fk (CSV parse) → awk (filter) → fk (enrich with fk builtins):"
show_pipe "$FK -i csv -H '{ print \$product, \$revenue, \$region }' $TMPDIR/sales.csv | awk '\$2+0 > 15000' | $FK '{ printf \"  %-8s \$%-6s (%s)  hex=\$%s\n\", \$1, \$2, \$3, hex(\$2+0) }'"

echo ""
echo "awk (generate data) → fk (bar chart with repeat):"
show_pipe "awk 'BEGIN { srand(42); for (i=1;i<=6;i++) printf \"%s %d\n\", \"item_\" i, int(rand()*50)+1 }' | $FK '{ printf \"  %-8s %s (%d)\n\", \$1, repeat(\"▓\", \$2), \$2 }'"

section "4. fk + paste + diff — structural comparison"

echo "Side-by-side: product names vs revenue:"
show_pipe "paste <($FK -i csv -H '{ print \$product }' $TMPDIR/sales.csv) <($FK -i csv -H '{ print \"\$\" \$revenue }' $TMPDIR/sales.csv) | $FK '{ printf \"  %-10s %s\n\", \$1, \$2 }'"

echo ""
echo "Diff two transformations of the same data:"
diff --color=never \
    <($FK -i csv -H '{ print $region, $product }' "$TMPDIR/sales.csv" | sort) \
    <($FK -i csv -H '$revenue+0 > 15000 { print $region, $product }' "$TMPDIR/sales.csv" | sort) \
    | $FK '{ print "  " $0 }' || true
echo "  (lines starting with < were filtered out by revenue > 15000)"

section "5. fk + xargs — parallel processing"

echo "Extract unique IPs, then resolve each (simulated):"
show_pipe "$FK '{ ips[\$1]++ } END { for (ip in ips) print ip }' $TMPDIR/access.log | xargs -I{} printf '  resolve {} → {}.example.com\n'"

printf "\n${C_BOLD}Done.${C_RESET} fk is a Unix citizen — composes freely with sort, awk, paste, diff, xargs.\n"
