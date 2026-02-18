#!/usr/bin/env bash
# 06 — Mini ETL: CSV → aggregate → report + stats
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "Mini ETL — CSV → aggregate → report + stats"
# Combines: -H, named columns, parsedate, strftime, trim, stats.
# Format auto-detected from .csv extension.

show $FK -H '
{
    ts = parsedate($created_at, "%Y-%m-%d %H:%M:%S")
    month = strftime("%Y-%m", ts)
    cust = trim($customer); amt = $amount + 0
    amounts[NR] = amt
    total += amt; count++
    by_cust[cust] += amt; n_cust[cust]++
    by_month[month] += amt
    if (amt > mx) { mx = amt; mx_id = $order_id; mx_who = cust }
}
END {
    printf "  Total: $%.2f across %d orders\n\n", total, count
    print "  By customer:"
    for (c in by_cust) cl[c] = sprintf("    %s %d orders  $%8.2f", rpad(c, 16), n_cust[c], by_cust[c])
    asort(cl); print cl
    print "\n  By month:"
    for (m in by_month) ml[m] = sprintf("    %s  $%8.2f", m, by_month[m])
    asort(ml); print ml
    printf "\n  Largest: #%s by %s ($%.2f)\n", mx_id, mx_who, mx
    printf "\n  Order stats:\n"
    printf "    mean=$%.2f  median=$%.2f  stddev=$%.2f\n", mean(amounts), median(amounts), stddev(amounts)
    printf "    iqm=$%.2f  p25=$%.2f  p75=$%.2f  p95=$%.2f\n", iqm(amounts), p(amounts,25), p(amounts,75), p(amounts,95)
}' "$TMPDIR/orders.csv"

printf "\n${C_BOLD}Done.${C_RESET} Full ETL pipeline in one fk invocation.\n"
