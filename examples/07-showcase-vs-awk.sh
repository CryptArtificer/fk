#!/usr/bin/env bash
# 07-showcase-vs-awk.sh — What fk can do that awk can't
#
# A self-documenting showcase: each example prints the fk command
# before running it. uplot charts are shown when uplot is installed.
#
# Run: ./examples/07-showcase-vs-awk.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

HAS_UPLOT=false
command -v uplot >/dev/null 2>&1 && HAS_UPLOT=true

# ── Display helpers ─────────────────────────────────────────────
C_RESET=$'\033[0m'
C_BOLD=$'\033[1m'
C_DIM=$'\033[2m'
C_CYAN=$'\033[1;96m'
C_YEL=$'\033[93m'
C_FLAG=$'\033[33m'
C_FILE=$'\033[37m'
C_SEC=$'\033[1;36m'

section() { printf "\n${C_SEC}━━ %s ━━${C_RESET}\n\n" "$1"; }

# Print the full fk command (program + switches), then run it.
show() {
    local flags="" prog="" files="" found_prog=false
    for arg in "$@"; do
        if [[ "$arg" == "$FK" ]]; then
            continue
        elif [[ "$found_prog" == false && "$arg" == -* ]]; then
            flags+=" ${C_FLAG}${arg}${C_RESET}"
        elif [[ "$found_prog" == false && ("$arg" == *"{"* || "$arg" == *"/"* && -f "$arg") ]]; then
            if [[ -f "$arg" ]]; then
                files+=" ${C_FILE}$(basename "$arg")${C_RESET}"
            else
                prog="$arg"
                found_prog=true
            fi
        elif $found_prog && [[ -f "$arg" ]]; then
            files+=" ${C_FILE}$(basename "$arg")${C_RESET}"
        elif $found_prog; then
            files+=" ${C_FILE}${arg}${C_RESET}"
        else
            flags+=" ${C_FLAG}${arg}${C_RESET}"
        fi
    done

    # Header: $ fk [flags] [files]
    printf "\n  ${C_DIM}\$${C_RESET} ${C_CYAN}${C_BOLD}fk${C_RESET}%s" "$flags"
    [[ -n "$files" ]] && printf " %b" "$files"

    # Program body — indented, yellow, with leading ' and trailing '
    if [[ -n "$prog" ]]; then
        printf " ${C_YEL}'${C_RESET}\n"
        while IFS= read -r line; do
            printf "    ${C_YEL}%s${C_RESET}\n" "$line"
        done <<< "$prog"
        printf "  ${C_YEL}'${C_RESET}\n"
    else
        printf "\n"
    fi
    echo ""
    "$@"
}

# show_pipe: print a readable pipeline description, then run it via eval
show_pipe() {
    local desc="$1"
    local display="${desc//$FK/fk}"
    display="${display//$TMPDIR\//}"
    printf "\n  ${C_DIM}\$${C_RESET} ${C_YEL}%s${C_RESET}\n\n" "$display"
    eval "$desc"
}

# ═══════════════════════════════════════════════════════════════
# Test data
# ═══════════════════════════════════════════════════════════════
cat > "$TMPDIR/sales.csv" <<'CSV'
region,product,revenue,units,quarter
EMEA,Widget,14500,230,Q1
APAC,Gadget,22300,410,Q2
NA,Widget,18700,350,Q1
EMEA,Gadget,9100,180,Q3
NA,Gizmo,31200,520,Q2
APAC,Widget,11800,200,Q4
EMEA,Gizmo,27400,460,Q3
NA,Gadget,16900,290,Q1
APAC,Gizmo,8600,150,Q4
CSV

cat > "$TMPDIR/access.log" <<'LOG'
192.168.1.10 - - [16/Feb/2025:10:15:30 +0000] "GET /index.html HTTP/1.1" 200 1234
10.0.0.5 - admin [16/Feb/2025:10:15:31 +0000] "POST /api/login HTTP/1.1" 302 0
172.16.0.1 - - [16/Feb/2025:10:15:32 +0000] "GET /static/style.css HTTP/1.1" 200 8901
192.168.1.10 - - [16/Feb/2025:10:15:33 +0000] "GET /api/data HTTP/1.1" 500 45
10.0.0.5 - admin [16/Feb/2025:10:15:34 +0000] "DELETE /api/users/3 HTTP/1.1" 204 0
192.168.1.10 - - [16/Feb/2025:10:15:35 +0000] "GET /api/data HTTP/1.1" 200 2048
172.16.0.1 - - [16/Feb/2025:10:15:36 +0000] "GET /api/users HTTP/1.1" 200 5120
10.0.0.5 - admin [16/Feb/2025:10:15:37 +0000] "PUT /api/users/3 HTTP/1.1" 200 128
LOG

cat > "$TMPDIR/events.csv" <<'CSV'
event,date,attendees
Kickoff,2025-01-15 09:00:00,45
Sprint Review,2025-02-01 14:00:00,30
Release Party,2025-02-16 18:00:00,120
Retrospective,2025-03-01 10:30:00,25
Offsite,2025-03-15 08:00:00,80
Hackathon,2025-04-01 10:00:00,60
CSV

cat > "$TMPDIR/servers.csv" <<'CSV'
host-name,cpu-usage,mem-usage,disk.free,net.rx-bytes
web-01,72.5,85.3,120,984320
web-02,45.1,60.2,250,1230400
db-01,91.8,95.1,50,540200
cache-01,12.3,40.7,180,320100
worker-01,88.2,78.9,90,2100000
worker-02,34.5,52.1,300,1800000
CSV

cat > "$TMPDIR/api.jsonl" <<'JSONL'
{"ts":"2025-02-16T10:00:01","method":"GET","path":"/api/users","status":200,"ms":12}
{"ts":"2025-02-16T10:00:02","method":"POST","path":"/api/users","status":201,"ms":45}
{"ts":"2025-02-16T10:00:03","method":"GET","path":"/api/users/42","status":200,"ms":8}
{"ts":"2025-02-16T10:00:04","method":"GET","path":"/api/products","status":500,"ms":1230}
{"ts":"2025-02-16T10:00:05","method":"DELETE","path":"/api/users/7","status":204,"ms":15}
{"ts":"2025-02-16T10:00:06","method":"GET","path":"/api/products","status":200,"ms":34}
{"ts":"2025-02-16T10:00:07","method":"POST","path":"/api/orders","status":201,"ms":89}
{"ts":"2025-02-16T10:00:08","method":"GET","path":"/api/users","status":200,"ms":11}
JSONL

printf "banana\napple\ncherry\ndate\nelderberry\nfig\ngrape\napricot\n" > "$TMPDIR/fruits.txt"

# ═══════════════════════════════════════════════════════════════
section "1. CSV with named columns — no csvkit, no counting fields"
# awk needs: csvkit to parse, then count which column is which.
# fk: -i csv -H gives you $column_name directly.

show $FK -i csv -H '{ rev[$region] += $revenue } END { for (r in rev) printf "  %-6s $%.0f\n", r, rev[r] }' "$TMPDIR/sales.csv"

# ═══════════════════════════════════════════════════════════════
section "2. Quoted column names — hyphens, dots, anything"
# awk: impossible. fk: \$\"col-name\" for any header.

show $FK -i csv -H '{
    cpu = $"cpu-usage" + 0; mem = $"mem-usage" + 0
    status = "OK"
    if (cpu > 90 || mem > 90) status = "CRITICAL"
    else if (cpu > 70 || mem > 70) status = "WARNING"
    printf "  %-12s cpu=%5.1f%%  mem=%5.1f%%  [%s]\n", $"host-name", cpu, mem, status
}' "$TMPDIR/servers.csv"

# ═══════════════════════════════════════════════════════════════
section "3. JSON Lines with jpath() — no jq needed"
# awk: zero JSON support. fk: jpath() navigates objects and arrays.

show $FK '{
    method = jpath($0, ".method"); status = jpath($0, ".status") + 0
    ms = jpath($0, ".ms") + 0; path = jpath($0, ".path")
    tag = ""
    if (status >= 500) tag = " ** ERROR"
    if (ms > 100) tag = tag " SLOW"
    printf "  %-6s %-20s %3d %4dms%s\n", method, path, status, ms, tag
}' "$TMPDIR/api.jsonl"

# ═══════════════════════════════════════════════════════════════
section "4. Nested JSON iteration"
# awk: completely impossible. fk: jpath with array extraction.

show_pipe "echo '{\"team\":\"eng\",\"members\":[{\"name\":\"Alice\",\"role\":\"lead\"},{\"name\":\"Bob\",\"role\":\"dev\"},{\"name\":\"Carol\",\"role\":\"dev\"}]}' | $FK '{
    team = jpath(\$0, \".team\")
    n = jpath(\$0, \".members\", m)
    for (i=1; i<=n; i++) printf \"  %s: %s (%s)\n\", team, jpath(m[i], \".name\"), jpath(m[i], \".role\")
}'"

# ═══════════════════════════════════════════════════════════════
section "5. Regex capture groups — structured log parsing"
# awk match() has no capture groups. fk: match(s, re, arr).

show $FK '{
    match($0, "^([0-9.]+) .* \"([A-Z]+) ([^ ]+) .*\" ([0-9]+) ([0-9]+)", c)
    printf "  %-15s %-6s %-25s %s  %sB\n", c[1], c[2], c[3], c[4], c[5]
}' "$TMPDIR/access.log"

# ═══════════════════════════════════════════════════════════════
section "6. Sorting — asort, asorti, join"
# awk: no built-in sort. fk: asort (by value), asorti (by key), join.

echo "Sort values alphabetically:"
show $FK '{ a[NR] = $0 } END { asort(a); for (i=1; i<=NR; i++) printf "  %s\n", a[i] }' "$TMPDIR/fruits.txt"

echo ""
echo "Sort CSV by revenue (descending) — fk does the work, not sort(1):"
show $FK -i csv -H '{
    rev[NR] = $revenue + 0
    line[NR] = sprintf("%-8s %-8s $%s", $region, $product, $revenue)
}
END {
    for (i = 1; i <= NR; i++) order[i] = i
    # Bubble sort by revenue descending (small dataset)
    for (i = 1; i <= NR; i++)
        for (j = i+1; j <= NR; j++)
            if (rev[order[i]] < rev[order[j]]) {
                tmp = order[i]; order[i] = order[j]; order[j] = tmp
            }
    for (i = 1; i <= NR; i++) printf "  %s\n", line[order[i]]
}' "$TMPDIR/sales.csv"

echo ""
echo "Sort keys + join into a single line:"
show_pipe "echo 'cherry apple banana date' | $FK '{ for (i=1;i<=NF;i++) a[i]=\$i; asort(a); print \"  \" join(a, \" → \") }'"

if $HAS_UPLOT; then
    echo ""
    echo "Revenue by region (sorted bar chart via uplot):"
    $FK -i csv -H '{ rev[$region] += $revenue } END { for (r in rev) printf "%s\t%.0f\n", r, rev[r] }' "$TMPDIR/sales.csv" | \
        sort -t$'\t' -k2 -n -r | uplot bar -t "Revenue by Region" -c cyan
fi

# ═══════════════════════════════════════════════════════════════
section "7. Date parsing + formatting"
# POSIX awk: no date functions at all. fk: parsedate, strftime, mktime.

show $FK -i csv -H '{
    ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
    dow = strftime("%A", ts)
    short = strftime("%b %d", ts)
    printf "  %-20s %-10s %-6s  %d people\n", $event, dow, short, $attendees
}' "$TMPDIR/events.csv"

if $HAS_UPLOT; then
    echo ""
    echo "Attendees timeline (line chart):"
    $FK -i csv -H '{
        ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
        day = strftime("%m/%d", ts)
        printf "%s\t%d\n", day, $attendees
    }' "$TMPDIR/events.csv" | uplot bar -t "Attendees by Event Date" -c green
fi

# ═══════════════════════════════════════════════════════════════
section "8. String toolkit — trim, reverse, chr, ord, hex"
# awk: none of these exist. You'd write 10-line functions for each.

show_pipe "printf '  hello world  \n  fk rocks  \n' | $FK '{ printf \"  trim: %-15s  rev: %s\n\", \"\\\"\" trim(\$0) \"\\\"\", reverse(trim(\$0)) }'"

echo ""
echo "ASCII table via chr():"
show_pipe "echo | $FK 'BEGIN { printf \"  \"; for (i=33; i<=126; i++) printf \"%s\", chr(i); print \"\" }'"

echo ""
echo "Encode/decode:"
show_pipe "echo 'Hello' | $FK '{ for (i=1; i<=length(\$0); i++) printf \"  %s → ord=%d hex=%s\n\", substr(\$0,i,1), ord(substr(\$0,i,1)), hex(ord(substr(\$0,i,1))) }'"

# ═══════════════════════════════════════════════════════════════
section "9. gensub — functional string replacement"
# awk: sub/gsub modify in place. fk: gensub returns a copy.

show_pipe "echo 'user=alice email=alice@example.com token=abc123secret' | $FK '{
    safe = gensub(\"token=[^ ]+\", \"token=***\", \"g\")
    safe = gensub(\"[a-zA-Z0-9.]+@[a-zA-Z0-9.]+\", \"***@***\", \"g\", safe)
    print \"  original:\", \$0
    print \"  redacted:\", safe
}'"

echo ""
echo "Replace only the 2nd occurrence:"
show_pipe "echo 'one-two-three-four-five' | $FK '{ print \"  \" gensub(\"-\", \"=DASH=\", 2) }'"

# ═══════════════════════════════════════════════════════════════
section "10. Bitwise operations + hex"
# awk: no bitwise ops, no hex output. fk: and/or/xor/lshift/rshift + hex.

echo "Unix permission decoder:"
show_pipe "printf '511\n493\n420\n256\n' | $FK '{
    mode = \$1+0; u=and(rshift(mode,6),7); g=and(rshift(mode,3),7); o=and(mode,7)
    printf \"  %3d → %d%d%d  hex=%s\n\", mode, u, g, o, hex(mode)
}'"

# ═══════════════════════════════════════════════════════════════
section "11. typeof() + negative fields"
# awk: no type introspection, no negative indexes.

show_pipe "echo | $FK 'BEGIN { x=42; y=\"hi\"; z[1]=\"a\"
    printf \"  %-6s type=%s\n\", \"42\", typeof(x)
    printf \"  %-6s type=%s\n\", \"\\\"hi\\\"\", typeof(y)
    printf \"  %-6s type=%s\n\", \"z[]\", typeof(z)
    printf \"  %-6s type=%s\n\", \"?\", typeof(w)
}'"

echo ""
echo "Negative field indexes (\$-1 = last, \$-2 = second-to-last):"
show_pipe "echo 'alpha beta gamma delta epsilon' | $FK '{ printf \"  last=%-10s 2nd-last=%s\n\", \$-1, \$-2 }'"

# ═══════════════════════════════════════════════════════════════
section "12. Piping: fk | fk — multi-stage pipelines"

echo "Stage 1 (CSV parse + filter) → Stage 2 (aggregate):"
show_pipe "$FK -i csv -H '\$revenue+0 > 15000 { print \$region, \$product, \$revenue }' $TMPDIR/sales.csv | $FK '{ by[\$1] += \$3 } END { for (r in by) printf \"  %-6s \$%.0f\n\", r, by[r] }'"

echo ""
echo "Three-stage: generate → compute → filter:"
show_pipe "printf '1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n' | $FK '{ print \$1, \$1**2, \$1**3 }' | $FK '\$2 > 20 { printf \"  n=%-2d  n²=%-4d  n³=%d\n\", \$1, \$2, \$3 }'"

# ═══════════════════════════════════════════════════════════════
section "13. fk + sort + uniq — classic Unix pipeline, supercharged"
# awk can do some of this, but fk's capture groups + named columns
# make the extraction step trivial.

echo "Top request paths by count (fk extracts, sort+uniq counts, fk formats):"
show_pipe "$FK '{ match(\$0, \"\\\"[A-Z]+ ([^ ]+)\", c); print c[1] }' $TMPDIR/access.log | sort | uniq -c | sort -rn | $FK '{ printf \"  %3d  %s\n\", \$1, \$2 }'"

echo ""
echo "Unique IPs with request count (fk parses, sort -u deduplicates):"
show_pipe "$FK '{ print \$1 }' $TMPDIR/access.log | sort | uniq -c | sort -rn | $FK '{ printf \"  %-15s %d requests\n\", \$2, \$1 }'"

# ═══════════════════════════════════════════════════════════════
section "14. fk + awk — interop both directions"
# fk output is awk-compatible. Mix tools freely.

echo "fk (CSV parse) → awk (filter) → fk (enrich with fk builtins):"
show_pipe "$FK -i csv -H '{ print \$product, \$revenue, \$region }' $TMPDIR/sales.csv | awk '\$2+0 > 15000' | $FK '{ printf \"  %-8s \$%-6s (%s)  hex=\$%s\n\", \$1, \$2, \$3, hex(\$2+0) }'"

echo ""
echo "awk (generate data) → fk (bar chart with repeat):"
show_pipe "awk 'BEGIN { srand(42); for (i=1;i<=6;i++) printf \"%s %d\n\", \"item_\" i, int(rand()*50)+1 }' | $FK '{ printf \"  %-8s %s (%d)\n\", \$1, repeat(\"▓\", \$2), \$2 }'"

# ═══════════════════════════════════════════════════════════════
section "15. fk + paste + diff — structural comparison"
# Combine fk's CSV parsing with paste for side-by-side or diff for changes.

echo "Side-by-side: product names vs revenue (paste combines fk outputs):"
show_pipe "paste <($FK -i csv -H '{ print \$product }' $TMPDIR/sales.csv) <($FK -i csv -H '{ print \"\$\" \$revenue }' $TMPDIR/sales.csv) | $FK '{ printf \"  %-10s %s\n\", \$1, \$2 }'"

echo ""
echo "Diff two transformations of the same data:"
diff --color=never \
    <($FK -i csv -H '{ print $region, $product }' "$TMPDIR/sales.csv" | sort) \
    <($FK -i csv -H '$revenue+0 > 15000 { print $region, $product }' "$TMPDIR/sales.csv" | sort) \
    | $FK '{ print "  " $0 }' || true
echo "  (lines starting with < were filtered out by revenue > 15000)"

# ═══════════════════════════════════════════════════════════════
section "16. fk + xargs — parallel processing"
# fk extracts, xargs parallelises.

echo "Extract unique IPs, then resolve each (simulated with printf):"
show_pipe "$FK '{ ips[\$1]++ } END { for (ip in ips) print ip }' $TMPDIR/access.log | xargs -I{} printf '  resolve {} → {}.example.com\n'"

# ═══════════════════════════════════════════════════════════════
section "17. Mini ETL — CSV → aggregate → report"
# Combines: -i csv, -H, named columns, parsedate, strftime, trim.

cat > "$TMPDIR/orders.csv" <<'CSV'
order_id,customer,amount,currency,created_at
1001,Alice Smith,149.99,USD,2025-01-10 08:30:00
1002,Bob Jones,2340.00,EUR,2025-01-15 14:20:00
1003,Carol Wu,89.50,USD,2025-02-01 09:00:00
1004,Alice Smith,320.00,USD,2025-02-05 16:45:00
1005,Dan Lee,1100.00,GBP,2025-02-10 11:30:00
1006,Bob Jones,450.00,EUR,2025-02-14 13:00:00
1007,Eve Park,75.00,USD,2025-02-15 10:15:00
1008,Alice Smith,210.00,USD,2025-02-16 08:00:00
CSV

show $FK -i csv -H '
{
    ts = parsedate($created_at, "%Y-%m-%d %H:%M:%S")
    month = strftime("%Y-%m", ts)
    cust = trim($customer); amt = $amount + 0
    total += amt; count++
    by_cust[cust] += amt; n_cust[cust]++
    by_month[month] += amt
    if (amt > mx) { mx = amt; mx_id = $order_id; mx_who = cust }
}
END {
    printf "  Total: $%.2f across %d orders\n\n", total, count
    print "  By customer:"
    for (c in by_cust) printf "    %-16s %d orders  $%8.2f\n", c, n_cust[c], by_cust[c]
    print "\n  By month:"
    for (m in by_month) printf "    %s  $%8.2f\n", m, by_month[m]
    printf "\n  Largest: #%s by %s ($%.2f)\n", mx_id, mx_who, mx
}' "$TMPDIR/orders.csv"

# ═══════════════════════════════════════════════════════════════
# Conditional uplot section
# ═══════════════════════════════════════════════════════════════
if $HAS_UPLOT; then

section "18. fk + uplot — terminal charts (uplot detected)"

echo "Latency histogram from access log:"
$FK '{ match($0, "\"[A-Z]+ [^ ]+ .*\" ([0-9]+) ([0-9]+)", c); print c[2]+0 }' "$TMPDIR/access.log" | \
    uplot hist -n 8 -t "Response Bytes" -c yellow

echo ""
echo "Revenue by product (bar chart):"
$FK -i csv -H '{ rev[$product] += $revenue } END { for (p in rev) printf "%s\t%.0f\n", p, rev[p] }' "$TMPDIR/sales.csv" | \
    sort -t$'\t' -k2 -n -r | uplot bar -t "Revenue by Product" -c cyan

echo ""
echo "Server CPU usage (bar chart with named columns):"
$FK -i csv -H '{ printf "%s\t%s\n", $"host-name", $"cpu-usage" }' "$TMPDIR/servers.csv" | \
    sort -t$'\t' -k2 -n -r | uplot bar -t "CPU Usage (%)" -c red

echo ""
echo "Event attendance (bar chart with dates from parsedate):"
$FK -i csv -H '{
    ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
    printf "%s (%s)\t%d\n", $event, strftime("%b %d", ts), $attendees
}' "$TMPDIR/events.csv" | uplot bar -t "Event Attendance" -c green

echo ""
echo "API latency by endpoint (jpath + uplot):"
$FK '{
    path = jpath($0, ".path"); ms = jpath($0, ".ms") + 0
    printf "%s\t%d\n", path, ms
}' "$TMPDIR/api.jsonl" | uplot box -t "API Latency by Endpoint (ms)"

else
    section "18. uplot not installed — skipping terminal charts"
    echo "  Install with: gem install youplot"
    echo "  Re-run to see bar charts, histograms, box plots, and scatter plots."
fi

# ═══════════════════════════════════════════════════════════════
printf "\n\033[1;32m━━ Done! ━━\033[0m\n"
echo "Every example uses fk features with no direct awk equivalent."
if $HAS_UPLOT; then
    echo "Charts rendered with uplot (gem install youplot)."
fi
