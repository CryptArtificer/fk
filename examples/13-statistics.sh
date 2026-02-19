#!/usr/bin/env bash
# 13 — Built-in statistics: mean, median, stddev, percentiles, IQM
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. Full stats summary — one function call each"
# awk: you'd write 50+ lines for basic stats. fk: one function call each.

echo "From raw latency data:"
show $FK '{
    a[NR] = $1
}
END {
    printf "  n       = %d\n", length(a)
    printf "  sum     = %.0f\n", sum(a)
    printf "  mean    = %.2f\n", mean(a)
    printf "  median  = %.2f\n", median(a)
    printf "  stddev  = %.2f\n", stddev(a)
    printf "  var     = %.2f\n", variance(a)
    printf "  min     = %.0f\n", min(a)
    printf "  max     = %.0f\n", max(a)
    printf "  p25     = %.2f\n", p(a, 25)
    printf "  p50     = %.2f\n", p(a, 50)
    printf "  p75     = %.2f\n", p(a, 75)
    printf "  p95     = %.2f\n", p(a, 95)
    printf "  p99     = %.2f\n", p(a, 99)
    printf "  iqm     = %.2f  (outlier-robust mean)\n", iqm(a)
}' "$TMPDIR/latencies.txt"

section "2. Per-column stats from CSV with named columns"

show $FK -H '{
    cpu[NR] = $"cpu-usage" + 0
    mem[NR] = $"mem-usage" + 0
}
END {
    printf "  CPU: mean=%5.1f%%  median=%5.1f%%  stddev=%5.1f%%  p95=%5.1f%%\n", mean(cpu), median(cpu), stddev(cpu), p(cpu,95)
    printf "  MEM: mean=%5.1f%%  median=%5.1f%%  stddev=%5.1f%%  p95=%5.1f%%\n", mean(mem), median(mem), stddev(mem), p(mem,95)
}' "$TMPDIR/servers.csv"

section "3. Revenue stats with named columns"

show $FK -H '{
    rev[$region] += $revenue
    all[NR] = $revenue + 0
}
END {
    for (r in rev) out[r] = sprintf("  %s $%.0f", rpad(r, 6), rev[r])
    asort(out); print out
    printf "\n  Revenue stats:  mean=$%.0f  median=$%.0f  stddev=$%.0f  p95=$%.0f\n", mean(all), median(all), stddev(all), p(all, 95)
}' "$TMPDIR/sales.csv"

section "4. API latency percentiles from JSON"

show $FK '{
    ms = jpath($0, ".ms") + 0; lat[NR] = ms
}
END {
    printf "  Latency: mean=%.0fms  median=%.0fms  p95=%.0fms  p99=%.0fms  max=%.0fms\n", mean(lat), median(lat), p(lat,95), p(lat,99), max(lat)
}' "$TMPDIR/api.jsonl"

section "5. Histogram — distribution at a glance"

echo "One-liner — just the data, everything else is automatic."
echo "Title and subtitle are derived from the source expression and filename:"
show $FK '{
    ms = jpath($0, ".ms") + 0; lat[NR] = ms
}
END {
    print plotbox(hist(lat))
}' "$TMPDIR/api.jsonl"

echo ""
echo "User title plus auto-subtitle:"
show $FK '{
    ms = jpath($0, ".ms") + 0; lat[NR] = ms
}
END {
    print plotbox(hist(lat, 6), 28, "▇", 0, "Latency (ms)", "Frequency", "yellow")
}' "$TMPDIR/api.jsonl"

printf "\n${C_BOLD}Done.${C_RESET} 15 stats builtins — no awk equivalent.\n"
