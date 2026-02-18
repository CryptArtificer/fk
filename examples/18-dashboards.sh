#!/usr/bin/env bash
# 18 — fk + uplot: terminal dashboards from AWS-style logs
#
# Generates realistic ALB & Lambda log data with fk, then aggregates
# and pipes to uplot for terminal visualisation. No Python, no R — just pipes.
#
# Requires: uplot (gem install youplot)
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"

if ! $HAS_UPLOT; then
    echo "Skipping: uplot not found. Install with: gem install youplot"
    exit 0
fi

section "Generating synthetic AWS logs with fk..."

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

section "1. Requests per hour (ALB traffic shape)"

$FK -F$'\t' '{
    match($1, "T([0-9]{2}):", c)
    hour[c[1]+0]++
}
END {
    for (h = 0; h <= 23; h++)
        printf "h%02d\t%d\n", h, hour[h]+0
}' "$TMPDIR/alb.tsv" | uplot bar -t "Requests per Hour" -c cyan

section "2. HTTP status code breakdown"

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

section "3. Response latency distribution"

$FK -F$'\t' '{ print $5 + 0 }' "$TMPDIR/alb.tsv" | \
    uplot hist -n 40 -t "Latency (ms) — most fast, long tail" -c yellow

section "4. p95 latency by endpoint"

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
        printf "%s\t%.0f\n", path, p(vals, 95)
        delete vals
    }
}' "$TMPDIR/alb.tsv" | sort -t$'\t' -k2 -n -r | \
    uplot bar -t "p95 Latency by Endpoint (ms)" -c red

section "5. 5xx errors over the day"

$FK -F$'\t' '{
    s = $4 + 0
    if (s >= 500) {
        match($1, "T([0-9]{2}):", c)
        hour[c[1]+0]++
    }
}
END {
    for (h = 0; h <= 23; h++)
        printf "%d\t%d\n", h, hour[h]+0
}' "$TMPDIR/alb.tsv" | uplot line -t "5xx Errors by Hour" -c red --xlabel "Hour" --ylabel "Count"

section "6. Lambda duration by function (boxplot)"

$FK -F$'\t' '{ printf "%s\t%d\n", $2, $3 }' "$TMPDIR/lambda.tsv" | \
    uplot box -t "Lambda Duration (ms) by Function"

section "7. Lambda errors by function and type"

$FK -F$'\t' '$7 != "none" {
    key = $2 " " $7
    errors[key]++
}
END {
    for (k in errors) printf "%s\t%d\n", k, errors[k]
}' "$TMPDIR/lambda.tsv" | sort -t$'\t' -k2 -n -r | \
    uplot bar -t "Lambda Errors (function + type)" -c magenta

section "Summary — computed entirely by fk"

$FK -F$'\t' '{
    latency = $5 + 0; status = $4 + 0
    lat[NR] = latency
    total++
    if (status >= 500) err5xx++
    if (status >= 400 && status < 500) err4xx++
    methods[$2]++
}
END {
    printf "  ALB: %d requests\n", total
    printf "  Latency: mean=%.0fms  median=%.0fms  p95=%.0fms  p99=%.0fms  max=%.0fms\n", mean(lat), median(lat), p(lat,95), p(lat,99), max(lat)
    printf "  IQM latency: %.0fms  (robust to outlier spikes)\n", iqm(lat)
    printf "  5xx: %d (%.1f%%)  4xx: %d (%.1f%%)\n", err5xx+0, (err5xx+0)*100/total, err4xx+0, (err4xx+0)*100/total
    printf "  Methods: "
    for (m in methods) printf "%s=%d ", m, methods[m]
    print ""
}' "$TMPDIR/alb.tsv"

echo ""
$FK -F$'\t' '{
    total++; func = $2; dur = $3 + 0
    durations[NR] = dur
    if ($6 == "cold") colds++
    if ($7 != "none") errs++
    funcs[func]++
}
END {
    printf "  Lambda: %d invocations\n", total
    printf "  Duration: mean=%.0fms  median=%.0fms  p95=%.0fms  stddev=%.0fms\n", mean(durations), median(durations), p(durations,95), stddev(durations)
    printf "  Cold starts: %d (%.1f%%)  Errors: %d (%.1f%%)\n", colds+0, (colds+0)*100/total, errs+0, (errs+0)*100/total
    printf "  Functions: "
    for (f in funcs) printf "%s=%d ", f, funcs[f]
    print ""
}' "$TMPDIR/lambda.tsv"

printf "\n${C_BOLD}Done.${C_RESET} 7 plots + summary from fk + uplot. No Python needed.\n"
