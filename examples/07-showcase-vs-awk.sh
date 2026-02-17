#!/usr/bin/env bash
# 07-showcase-vs-awk.sh — What fk can do that awk can't
#
# Every example here uses features with no direct awk equivalent.
# Run: ./examples/07-showcase-vs-awk.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

section() { printf "\n\033[1;36m── %s ──\033[0m\n\n" "$1"; }

# ─────────────────────────────────────────────────────────────────
section "1. CSV analytics with named columns"
# awk: you'd pipe through csvkit, then awk -F, and count fields by hand.
# fk: first-class CSV + header mode. Column names just work.

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

echo "Revenue by region (CSV with named columns):"
$FK -i csv -H '
    { rev[$region] += $revenue; units[$region] += $units }
    END {
        for (r in rev)
            printf "  %-6s $%-8.0f  (%d units, $%.0f avg/unit)\n", r, rev[r], units[r], rev[r] / units[r]
    }
' "$TMPDIR/sales.csv"

echo ""
echo "Top product by total revenue:"
$FK -i csv -H '
    { rev[$product] += $revenue }
    END {
        best = ""; best_rev = 0
        for (p in rev)
            if (rev[p] > best_rev) { best = p; best_rev = rev[p] }
        printf "  %s — $%.0f\n", best, best_rev
    }
' "$TMPDIR/sales.csv"

# ─────────────────────────────────────────────────────────────────
section "2. JSON Lines — API log analysis with jpath()"
# awk: no JSON support at all. You'd need jq + awk in a pipeline.
# fk: jpath() navigates JSON objects and arrays natively.

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

echo "Request summary (from JSON Lines with jpath):"
$FK '{
    method = jpath($0, ".method")
    status = jpath($0, ".status") + 0
    ms     = jpath($0, ".ms") + 0
    path   = jpath($0, ".path")

    methods[method]++
    total_ms += ms; n++
    if (status >= 500) errors++
    if (ms > slow_ms) { slow_ms = ms; slow_path = path }
}
END {
    printf "  Requests: %d (avg %.0fms)\n", n, total_ms / n
    printf "  Errors:   %d (%.0f%%)\n", errors+0, (errors+0)*100/n
    printf "  Slowest:  %s (%dms)\n", slow_path, slow_ms
    printf "  Methods:  "
    for (m in methods) printf "%s=%d ", m, methods[m]
    print ""
}' "$TMPDIR/api.jsonl"

# ─────────────────────────────────────────────────────────────────
section "3. Nested JSON with jpath() iteration"
# awk: completely impossible without external tools.
# fk: jpath() navigates nested objects, arrays, supports iteration.

echo '{"cluster":"prod","nodes":[{"host":"web-1","cpu":72,"mem":85},{"host":"web-2","cpu":45,"mem":60},{"host":"db-1","cpu":91,"mem":95}]}' | \
$FK '{
    cluster = jpath($0, ".cluster")
    n = jpath($0, ".nodes", nodes)
    printf "  Cluster: %s (%d nodes)\n", cluster, n
    for (i = 1; i <= n; i++) {
        host = jpath(nodes[i], ".host")
        cpu  = jpath(nodes[i], ".cpu") + 0
        mem  = jpath(nodes[i], ".mem") + 0
        flag = ""
        if (cpu > 90 || mem > 90) flag = " *** HIGH"
        printf "    %-8s cpu=%d%%  mem=%d%%%s\n", host, cpu, mem, flag
    }
}'

# ─────────────────────────────────────────────────────────────────
section "4. Regex capture groups — structured extraction"
# awk: match() sets RSTART/RLENGTH but has no capture groups.
# fk: match(str, regex, arr) fills arr[1], arr[2], ... with groups.

cat > "$TMPDIR/access.log" <<'LOG'
192.168.1.10 - - [16/Feb/2025:10:15:30 +0000] "GET /index.html HTTP/1.1" 200 1234
10.0.0.5 - admin [16/Feb/2025:10:15:31 +0000] "POST /api/login HTTP/1.1" 302 0
172.16.0.1 - - [16/Feb/2025:10:15:32 +0000] "GET /static/style.css HTTP/1.1" 200 8901
192.168.1.10 - - [16/Feb/2025:10:15:33 +0000] "GET /api/data HTTP/1.1" 500 45
10.0.0.5 - admin [16/Feb/2025:10:15:34 +0000] "DELETE /api/users/3 HTTP/1.1" 204 0
LOG

echo "Parse Apache log with capture groups:"
$FK '{
    match($0, "^([0-9.]+) .* \"([A-Z]+) ([^ ]+) .*\" ([0-9]+) ([0-9]+)", c)
    ip = c[1]; method = c[2]; path = c[3]; status = c[4]+0; bytes = c[5]+0
    printf "  %-15s %-6s %-25s %d  %dB\n", ip, method, path, status, bytes
    if (status >= 500) errs++
}
END { printf "  → %d server errors\n", errs+0 }' "$TMPDIR/access.log"

# ─────────────────────────────────────────────────────────────────
section "5. Data pipeline: sort + join + histogram"
# awk: no asort, no join, no repeat for bar charts.
# fk: full array manipulation + string toolkit.

echo "Word frequency histogram:"
echo "the quick brown fox jumps over the lazy dog the fox the the" | $FK '{
    for (i = 1; i <= NF; i++) freq[$i]++
}
END {
    for (w in freq) counts[w] = freq[w]
    n = asorti(freq)
    for (i = 1; i <= n; i++) {
        word = freq[i]
        c = counts[word]
        printf "  %-8s %s (%d)\n", word, repeat("█", c), c
    }
}'

echo ""
echo "Sort by value and join:"
echo "cherry apple banana date" | $FK '{
    for (i = 1; i <= NF; i++) a[i] = $i
    asort(a)
    print "  sorted:", join(a, " → ")
}'

# ─────────────────────────────────────────────────────────────────
section "6. Multi-format date wrangling"
# awk: no date parsing, no strftime, no mktime in POSIX awk.
# fk: parsedate + strftime + mktime — all built in.

cat > "$TMPDIR/events.csv" <<'CSV'
event,date,attendees
Kickoff,2025-01-15 09:00:00,45
Sprint Review,2025-02-01 14:00:00,30
Release Party,2025-02-16 18:00:00,120
Retrospective,2025-03-01 10:30:00,25
CSV

echo "Event timeline with day-of-week:"
$FK -i csv -H '{
    ts = parsedate($date, "%Y-%m-%d %H:%M:%S")
    dow = strftime("%A", ts)
    ymd = strftime("%b %d", ts)
    printf "  %-20s %-10s %-6s  %d attendees\n", $event, dow, ymd, $attendees
}' "$TMPDIR/events.csv"

echo ""
echo "Epoch round-trip:"
echo | $FK 'BEGIN {
    epoch = mktime("2025 02 16 12 30 00")
    print "  mktime → " epoch
    print "  strftime → " strftime("%A, %B %d %Y at %H:%M UTC", epoch)
}'

# ─────────────────────────────────────────────────────────────────
section "7. Bitwise flags + hex output"
# awk: no bitwise operations, no hex(), no chr().
# fk: and/or/xor/lshift/rshift + hex + chr for binary protocol work.

echo "Unix permission bits from octal:"
printf "511\n493\n420\n256\n438\n" | $FK '{
    mode = $1 + 0
    u = and(rshift(mode, 6), 7)
    g = and(rshift(mode, 3), 7)
    o = and(mode, 7)
    printf "  %3d → %d%d%d (", mode, u, g, o
    bits = "rwx"
    for (shift = 8; shift >= 0; shift--) {
        if (and(mode, lshift(1, shift)))
            printf "%s", substr(bits, 3 - (shift % 3), 1)
        else
            printf "-"
    }
    printf ")  hex=%s\n", hex(mode)
}'

echo ""
echo "Flag check — which IPs have the 'admin' bit (0x04) set:"
printf "10.0.0.1 7\n10.0.0.2 3\n10.0.0.3 5\n10.0.0.4 12\n" | \
    $FK '{ if (and($2, 4)) printf "  %-12s flags=%s → admin\n", $1, hex($2) }'

# ─────────────────────────────────────────────────────────────────
section "8. String toolkit one-liners"
# awk: none of these exist — you'd write multi-line functions.
# fk: built-in trim, reverse, startswith, endswith, repeat, chr, ord, hex.

printf "  hello world  \n  fk is great  \n  trim me  \n" | \
    $FK '{ printf "  trim: %-15s  reverse: %s\n", "\"" trim($0) "\"", reverse(trim($0)) }'

echo ""
echo "ASCII table (32–126) via chr():"
echo | $FK 'BEGIN {
    printf "  "
    for (i = 32; i <= 126; i++) printf "%s", chr(i)
    print ""
    printf "  (%d printable characters)\n", 126 - 32 + 1
}'

echo ""
echo "String analysis:"
printf "Hello, World!\nfk-is-awesome\ncafé résumé\n" | $FK '{
    printf "  %-16s len=%-3d", $0, length($0)
    printf " starts_H=%d", startswith($0, "H")
    printf " ends_!=%d", endswith($0, "!")
    printf " rev=%s\n", reverse($0)
}'

# ─────────────────────────────────────────────────────────────────
section "9. Negative indexing + computed fields"
# awk: no negative field indexes, no $-1.
# fk: $-1 = last, $-2 = second-to-last, etc.

echo "Negative field indexes — access from the end:"
printf "alice 30 NYC engineer\nbob 25 LA designer\ncarol 35 Chicago manager\n" | $FK '{
    printf "  %-8s last=%-10s 2nd-last=%-8s\n", $1, $-1, $-2
}'

echo ""
echo "Reverse column order (computed + negative fields):"
echo "alpha beta gamma delta epsilon" | $FK '{
    for (i = NF; i >= 1; i--) printf "%s ", $i
    print ""
}'

# ─────────────────────────────────────────────────────────────────
section "10. gensub — functional string replacement"
# awk: sub/gsub modify in place, no way to get modified copy.
# fk: gensub returns the result, leaving $0 untouched.

echo "Sanitise sensitive fields without destroying the original:"
printf "user=alice email=alice@example.com token=abc123secret\n" | $FK '{
    safe = gensub("token=[^ ]+", "token=***", "g")
    safe = gensub("[a-zA-Z0-9.]+@[a-zA-Z0-9.]+", "***@***", "g", safe)
    print "  original:", $0
    print "  redacted:", safe
}'

echo ""
echo "Replace only the 2nd occurrence:"
echo "one-two-three-four-five" | $FK '{
    print "  original:", $0
    print "  2nd dash → DASH:", gensub("-", "=DASH=", 2)
}'

# ─────────────────────────────────────────────────────────────────
section "11. typeof() — runtime type introspection"
# awk: no type introspection at all.
# fk: typeof returns "number", "string", "array", "uninitialized".

echo | $FK 'BEGIN {
    x = 42; y = "hello"; z[1] = "a"; z[2] = "b"
    printf "  %-12s type=%-15s value=%s\n", "x=42", typeof(x), x
    printf "  %-12s type=%-15s value=%s\n", "y=\"hello\"", typeof(y), y
    printf "  %-12s type=%-15s (%d elements)\n", "z[1]=\"a\"", typeof(z), length(z)
    printf "  %-12s type=%s\n", "(unset)", typeof(missing)
}'

# ─────────────────────────────────────────────────────────────────
section "12. Mini ETL — CSV analytics report"
# Combines: -i csv, -H, named columns, parsedate, strftime,
# trim, gensub — all in one cohesive program.

cat > "$TMPDIR/orders.csv" <<'CSV'
order_id,customer,email,amount,currency,created_at
1001,Alice Smith,alice@example.com,149.99,USD,2025-01-10 08:30:00
1002,Bob Jones,bob@corp.io,2340.00,EUR,2025-01-15 14:20:00
1003,Carol Wu,carol@example.com,89.50,USD,2025-02-01 09:00:00
1004,Alice Smith,alice@example.com,320.00,USD,2025-02-05 16:45:00
1005,Dan Lee,dan@example.com,1100.00,GBP,2025-02-10 11:30:00
1006,Bob Jones,bob@corp.io,450.00,EUR,2025-02-14 13:00:00
1007,Eve Park,eve@startup.co,75.00,USD,2025-02-15 10:15:00
1008,Alice Smith,alice@example.com,210.00,USD,2025-02-16 08:00:00
CSV

echo "Order analytics report:"
$FK -i csv -H '
{
    ts = parsedate($created_at, "%Y-%m-%d %H:%M:%S")
    month = strftime("%Y-%m", ts)
    cust = trim($customer)
    amt = $amount + 0

    total += amt; count++
    by_cust[cust] += amt; orders_cust[cust]++
    by_month[month] += amt; cnt_month[month]++
    if (amt > max_amt) { max_amt = amt; max_id = $order_id; max_cust = cust }
}
END {
    printf "  Total: $%.2f across %d orders (avg $%.2f)\n", total, count, total/count
    print ""
    print "  By customer:"
    for (c in by_cust)
        printf "    %-18s %d orders  $%8.2f\n", c, orders_cust[c], by_cust[c]
    print ""
    print "  By month:"
    for (m in by_month)
        printf "    %s  %d orders  $%8.2f\n", m, cnt_month[m], by_month[m]
    printf "\n  Largest: #%s by %s ($%.2f)\n", max_id, max_cust, max_amt
}' "$TMPDIR/orders.csv"

# ─────────────────────────────────────────────────────────────────
section "13. Quoted column names — real-world headers"
# awk: no way to use column names with special characters.
# fk: $"col-name" accesses any header, hyphens/dots/spaces included.

cat > "$TMPDIR/metrics.csv" <<'CSV'
host-name,cpu-usage,mem-usage,disk.free,net.rx-bytes
web-01,72.5,85.3,120,984320
web-02,45.1,60.2,250,1230400
db-01,91.8,95.1,50,540200
cache-01,12.3,40.7,180,320100
CSV

echo "Server health check (using quoted column names):"
$FK -i csv -H '{
    host = $"host-name"
    cpu  = $"cpu-usage" + 0
    mem  = $"mem-usage" + 0
    disk = $"disk.free" + 0

    status = "OK"
    if (cpu > 90 || mem > 90) status = "CRITICAL"
    else if (cpu > 70 || mem > 70) status = "WARNING"
    else if (disk < 100) status = "LOW DISK"

    printf "  %-10s cpu=%5.1f%%  mem=%5.1f%%  disk=%dGB  [%s]\n", host, cpu, mem, disk, status
}' "$TMPDIR/metrics.csv"

echo ""
printf "\033[1;32m═══ Done! All examples use fk features with no awk equivalent. ═══\033[0m\n"
