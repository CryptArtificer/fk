#!/usr/bin/env bash
# 08-fk-uplot-aws.sh — fk + uplot: terminal dashboards from AWS-style logs
#
# Generates realistic AWS ALB & Lambda log data with fk, then aggregates
# and pipes to uplot for terminal visualisation. No Python, no R — just pipes.
#
# Requires: uplot (gem install youplot)
# Run: ./examples/08-fk-uplot-aws.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if ! command -v uplot >/dev/null 2>&1; then
    echo "uplot not found. Install with: gem install youplot"
    exit 1
fi

section() { printf "\n\033[1;35m━━ %s ━━\033[0m\n\n" "$1"; }

# ═════════════════════════════════════════════════════════════════
# Step 1: Generate realistic log data using fk
# (fk as a data generator — awk can't do strftime/mktime)
# ═════════════════════════════════════════════════════════════════

echo "Generating synthetic AWS logs with fk..."

# ── ALB access logs: 5000 requests over 24h ─────────────────────
echo | $FK 'BEGIN {
    srand(1337)
    split("GET GET GET GET GET POST POST PUT DELETE HEAD", methods, " ")
    split("/api/users /api/orders /api/products /api/search /api/auth /api/health /static/app.js /static/style.css", paths, " ")
    split("200 200 200 200 200 200 200 201 204 301 400 401 403 404 500 502 503", statuses, " ")

    base = mktime("2025 02 16 00 00 00")

    for (i = 1; i <= 5000; i++) {
        ts = base + int(rand() * 86400)
        method = methods[int(rand() * 10) + 1]
        path = paths[int(rand() * 8) + 1]
        status = statuses[int(rand() * 17) + 1]

        latency = int(rand() * 50 + 5)
        if (rand() < 0.1) latency = int(rand() * 500 + 100)
        if (rand() < 0.02) latency = int(rand() * 3000 + 1000)

        bytes = int(rand() * 5000 + 200)
        if (index(path, "static") > 0) bytes = int(rand() * 50000 + 10000)

        printf "%s\t%s\t%s\t%d\t%d\t%d\n", strftime("%Y-%m-%dT%H:%M:%S", ts), method, path, status, latency, bytes
    }
}' > "$TMPDIR/alb.tsv"

# ── Lambda invocation logs: 3000 invocations ────────────────────
echo | $FK 'BEGIN {
    srand(42)
    split("user-svc order-svc search-svc auth-svc notify-svc", funcs, " ")

    base = mktime("2025 02 16 00 00 00")

    for (i = 1; i <= 3000; i++) {
        ts = base + int(rand() * 86400)
        func = funcs[int(rand() * 5) + 1]

        duration = int(rand() * 100 + 10)
        if (rand() < 0.15) duration = int(rand() * 800 + 200)
        if (rand() < 0.03) duration = int(rand() * 5000 + 2000)

        mem_used = int(rand() * 200 + 64)
        mem_alloc = 256
        if (func == "search-svc") { mem_alloc = 512; mem_used = int(rand() * 400 + 100) }

        cold = (rand() < 0.05) ? "cold" : "warm"
        error = "none"
        if (rand() < 0.03) error = "timeout"
        if (rand() < 0.02) error = "oom"
        if (rand() < 0.01) error = "crash"

        printf "%s\t%s\t%d\t%d\t%d\t%s\t%s\n", strftime("%Y-%m-%dT%H:%M:%S", ts), func, duration, mem_used, mem_alloc, cold, error
    }
}' > "$TMPDIR/lambda.tsv"

echo "  → $(wc -l < "$TMPDIR/alb.tsv") ALB requests, $(wc -l < "$TMPDIR/lambda.tsv") Lambda invocations"

# ═════════════════════════════════════════════════════════════════
# Plot 1: Request rate by hour
# ═════════════════════════════════════════════════════════════════
section "1. Requests per hour (ALB traffic shape)"

$FK -F$'\t' '{
    match($1, "T([0-9]{2}):", c)
    hour[c[1]+0]++
}
END {
    for (h = 0; h <= 23; h++)
        printf "h%02d\t%d\n", h, hour[h]+0
}' "$TMPDIR/alb.tsv" | uplot bar -t "Requests per Hour" -c cyan

# ═════════════════════════════════════════════════════════════════
# Plot 2: HTTP status code distribution
# ═════════════════════════════════════════════════════════════════
section "2. HTTP status code breakdown (ALB)"

$FK -F$'\t' '{
    s = $4 + 0
    if (s >= 200 && s < 300) class = "2xx OK"
    else if (s >= 300 && s < 400) class = "3xx Redirect"
    else if (s >= 400 && s < 500) class = "4xx Client"
    else if (s >= 500) class = "5xx Server"
    else class = "other"
    counts[class]++
}
END {
    for (c in counts) printf "%s\t%d\n", c, counts[c]
}' "$TMPDIR/alb.tsv" | sort | uplot bar -t "HTTP Status Classes" -c green

# ═════════════════════════════════════════════════════════════════
# Plot 3: Latency distribution
# ═════════════════════════════════════════════════════════════════
section "3. Response latency distribution (ALB)"

$FK -F$'\t' '{ print $5 + 0 }' "$TMPDIR/alb.tsv" | \
    uplot hist -n 40 -t "Latency (ms) — most fast, long tail" -c yellow

# ═════════════════════════════════════════════════════════════════
# Plot 4: p95 latency per endpoint
# ═════════════════════════════════════════════════════════════════
section "4. p95 latency by endpoint (ALB)"

$FK -F$'\t' '
{
    path = $3; latency = $5 + 0
    n[path]++
    data[path, n[path]] = latency
}
END {
    for (path in n) {
        count = n[path]
        for (i = 1; i <= count; i++) vals[i] = data[path, i] + 0
        asort(vals)
        p95 = int(count * 0.95)
        if (p95 < 1) p95 = 1
        printf "%s\t%d\n", path, vals[p95]
        delete vals
    }
}' "$TMPDIR/alb.tsv" | sort -t$'\t' -k2 -n -r | \
    uplot bar -t "p95 Latency by Endpoint (ms)" -c red

# ═════════════════════════════════════════════════════════════════
# Plot 5: 5xx errors over time
# ═════════════════════════════════════════════════════════════════
section "5. Server errors (5xx) over the day (ALB)"

$FK -F$'\t' '{
    s = $4 + 0
    if (s >= 500) {
        match($1, "T([0-9]{2}):", c)
        hour[c[1]+0]++
    }
    total++
}
END {
    for (h = 0; h <= 23; h++)
        printf "%d\t%d\n", h, hour[h]+0
}' "$TMPDIR/alb.tsv" | uplot line -t "5xx Errors by Hour" -c red --xlabel "Hour" --ylabel "Count"

# ═════════════════════════════════════════════════════════════════
# Plot 6: Lambda duration by function (boxplot)
# ═════════════════════════════════════════════════════════════════
section "6. Lambda duration by function (boxplot)"

$FK -F$'\t' '{ printf "%s\t%d\n", $2, $3 }' "$TMPDIR/lambda.tsv" | \
    uplot box -t "Lambda Duration (ms) by Function"

# ═════════════════════════════════════════════════════════════════
# Plot 7: Lambda errors by function + type
# ═════════════════════════════════════════════════════════════════
section "7. Lambda errors by function and type"

$FK -F$'\t' '$7 != "none" {
    key = $2 " " $7
    errors[key]++
}
END {
    for (k in errors) printf "%s\t%d\n", k, errors[k]
}' "$TMPDIR/lambda.tsv" | sort -t$'\t' -k2 -n -r | \
    uplot bar -t "Lambda Errors (function + type)" -c magenta

# ═════════════════════════════════════════════════════════════════
# Plot 8: Memory utilisation scatter
# ═════════════════════════════════════════════════════════════════
section "8. Lambda memory: allocated vs used (scatter)"

$FK -F$'\t' '{ printf "%d\t%d\n", $5, $4 }' "$TMPDIR/lambda.tsv" | \
    uplot scatter -t "Memory: Allocated vs Used (MB)" -c cyan \
        --xlabel "Allocated" --ylabel "Used"

# ═════════════════════════════════════════════════════════════════
# Plot 9: Cold start rate by function
# ═════════════════════════════════════════════════════════════════
section "9. Cold start rate by Lambda function"

$FK -F$'\t' '{
    total[$2]++
    if ($6 == "cold") cold[$2]++
}
END {
    for (f in total) {
        pct = (cold[f]+0) * 100.0 / total[f]
        printf "%s\t%.1f\n", f, pct
    }
}' "$TMPDIR/lambda.tsv" | sort -t$'\t' -k2 -n -r | \
    uplot bar -t "Cold Start Rate (%) by Function" -c yellow

# ═════════════════════════════════════════════════════════════════
# Plot 10: Bandwidth by endpoint
# ═════════════════════════════════════════════════════════════════
section "10. Bandwidth by endpoint (MB)"

$FK -F$'\t' '{
    bytes[$3] += $6
}
END {
    for (p in bytes) printf "%s\t%.1f\n", p, bytes[p] / 1048576
}' "$TMPDIR/alb.tsv" | sort -t$'\t' -k2 -n -r | \
    uplot bar -t "Total Bandwidth by Endpoint (MB)" -c green

# ═════════════════════════════════════════════════════════════════
# Summary stats (pure fk, no uplot)
# ═════════════════════════════════════════════════════════════════
section "Summary — computed entirely by fk"

$FK -F$'\t' '{
    latency = $5 + 0; status = $4 + 0
    total++; sum_lat += latency
    if (latency > max_lat) max_lat = latency
    if (status >= 500) err5xx++
    if (status >= 400 && status < 500) err4xx++
    methods[$2]++
}
END {
    printf "  ALB: %d requests, avg latency %.0fms, max %dms\n", total, sum_lat/total, max_lat
    printf "  5xx: %d (%.1f%%)  4xx: %d (%.1f%%)\n", err5xx+0, (err5xx+0)*100/total, err4xx+0, (err4xx+0)*100/total
    printf "  Methods: "
    for (m in methods) printf "%s=%d ", m, methods[m]
    print ""
}' "$TMPDIR/alb.tsv"

echo ""
$FK -F$'\t' '{
    total++; func = $2; dur = $3 + 0; sum_dur += dur
    if ($6 == "cold") colds++
    if ($7 != "none") errs++
    funcs[func]++
}
END {
    printf "  Lambda: %d invocations, avg duration %.0fms\n", total, sum_dur/total
    printf "  Cold starts: %d (%.1f%%)  Errors: %d (%.1f%%)\n", colds+0, (colds+0)*100/total, errs+0, (errs+0)*100/total
    printf "  Functions: "
    for (f in funcs) printf "%s=%d ", f, funcs[f]
    print ""
}' "$TMPDIR/lambda.tsv"

# ═════════════════════════════════════════════════════════════════
printf "\n\033[1;32m━━ Done! 10 plots + summary from fk + uplot ━━\033[0m\n"
echo "Pipeline: fk generates → fk aggregates → uplot renders. No Python needed."
