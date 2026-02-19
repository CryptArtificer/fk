#!/usr/bin/env bash
# 15 — Pipelines: fk as a composable Unix tool
#
# Story: fk slots into pipelines naturally. Chain it with itself,
# with awk, with sort/uniq, or with anything that speaks stdin/stdout.
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. fk | fk — multi-stage processing"

echo "Stage 1: parse CSV and filter → Stage 2: aggregate:"
show_pipe "$FK -H '\$revenue+0 > 15000 { print \$region, \$product, \$revenue }' $TMPDIR/sales.csv |
  $FK '{ by[\$1] += \$3 } END { for (r in by) printf \"  %-6s \$%.0f\n\", r, by[r] }'"

echo ""
echo "Three stages: generate → compute → filter:"
show_pipe "seq 1 10 |
  $FK '{ print \$1, \$1**2, \$1**3 }' |
  $FK '\$2 > 20 { printf \"  n=%-2d  n²=%-4d  n³=%d\n\", \$1, \$2, \$3 }'"

section "2. fk + sort + uniq — the classic combo"

echo "Top request paths by frequency:"
show_pipe "$FK '{ match(\$0, \"\\\"[A-Z]+ ([^ ]+)\", c); print c[1] }' $TMPDIR/access.log |
  sort | uniq -c | sort -rn |
  $FK '{ printf \"  %3d  %s\n\", \$1, \$2 }'"

echo ""
echo "Unique IPs ranked by request count:"
show_pipe "$FK '{ print \$1 }' $TMPDIR/access.log |
  sort | uniq -c | sort -rn |
  $FK '{ printf \"  %-15s %d requests\n\", \$2, \$1 }'"

section "3. fk ↔ awk — interop in both directions"

echo "fk parses CSV → awk filters → fk formats with builtins:"
show_pipe "$FK -H '{ print \$product, \$revenue, \$region }' $TMPDIR/sales.csv |
  awk '\$2+0 > 15000' |
  $FK '{ printf \"  %-8s \$%-6s (%s)\n\", \$1, \$2, \$3 }'"

echo ""
echo "awk generates data → fk draws bar chart:"
show_pipe "awk 'BEGIN { srand(42); for (i=1;i<=6;i++) printf \"%s %d\n\", \"item_\" i, int(rand()*50)+1 }' |
  $FK '{ printf \"  %-8s %s (%d)\n\", \$1, repeat(\"▓\", \$2), \$2 }'"

section "4. Real pipeline — API health report from JSON logs"

echo "Stage 1: extract fields from JSON → Stage 2: per-endpoint stats:"
show_pipe "$FK -i json '{ print \$3, \$4+0, \$5+0 }' $TMPDIR/api.jsonl |
  $FK '{
    path=\$1; status=\$2+0; ms=\$3+0
    total[path]++; lat[path,total[path]]=ms
    if (status >= 400) errs[path]++
}
END {
    for (p in total) {
        for (i=1; i<=total[p]; i++) a[i]=lat[p,i]
        printf \"  %-20s %d reqs  p50=%3.0fms  p95=%3.0fms  errors=%d\n\", p, total[p], p(a,50), p(a,95), errs[p]+0
        delete a
    }
}'"

printf "\n${C_BOLD}Done.${C_RESET} fk composes freely with sort, awk, uniq, and itself.\n"
