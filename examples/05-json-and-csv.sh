#!/usr/bin/env bash
# 05-json-and-csv.sh — Structured input modes and jpath
#
# Run: ./examples/05-json-and-csv.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"

echo "═══ structured input ═══"
echo ""

# ── CSV input mode ───────────────────────────────────────────────
echo "1) CSV input (-i csv):"
CSV=$(cat <<'EOF'
name,city,score
"Alice",New York,95
"Bob","San Francisco",87
"Carol, Jr.",Boston,92
EOF
)
echo "$CSV" | $FK -i csv -H '{ printf "%-15s %-18s %s\n", $1, $2, $3 }'
echo ""

echo "   Quoted fields with commas handled correctly:"
echo "$CSV" | $FK -i csv 'NR > 1 { print NR-1 ": [" $1 "]" }'
echo ""

# ── TSV input mode ───────────────────────────────────────────────
echo "2) TSV input (-i tsv):"
printf "product\tqty\tprice\nWidget\t100\t9.99\nGadget\t50\t24.95\nGizmo\t200\t4.50\n" | \
    $FK -i tsv -H '{ printf "%-10s %3d units @ $%-6s = $%.2f\n", $1, $2, $3, $2 * $3 }'
echo ""

# ── Header mode ──────────────────────────────────────────────────
echo "3) Header mode (-H) — column names in HDR array:"
echo "$CSV" | $FK -i csv -H 'BEGIN { } { print "Row", NR-1, ": name=" $1, "score=" $3 }'
echo ""

# ── JSON lines input ─────────────────────────────────────────────
echo "4) JSON lines input (-i json):"
JSONL=$(cat <<'EOF'
{"name":"Alice","role":"engineer","level":3}
{"name":"Bob","role":"designer","level":2}
{"name":"Carol","role":"engineer","level":4}
{"name":"Dave","role":"manager","level":3}
EOF
)
echo "$JSONL" | $FK -i json '{ printf "%-8s %-10s L%d\n", $1, $2, $3 }'
echo ""

echo "   Filter JSON by field value:"
echo "$JSONL" | $FK -i json '$2 == "engineer" { print $1, "L" $3 }'
echo ""

# ── jpath — navigate nested JSON ─────────────────────────────────
echo "5) jpath() — drill into nested JSON:"
NESTED='{"server":{"host":"db.example.com","port":5432,"tags":["primary","us-east"]}}'
echo "$NESTED" | $FK '{
    print "Host:", jpath($0, ".server.host")
    print "Port:", jpath($0, ".server.port")
    print "Tags:", jpath($0, ".server.tags")
}'
echo ""

# ── jpath — iterate arrays ───────────────────────────────────────
echo "6) jpath() — iterate over arrays:"
USERS='{"users":[{"name":"Alice","id":101},{"name":"Bob","id":102},{"name":"Carol","id":103}]}'
echo "   All names: "
echo "$USERS" | $FK '{ print jpath($0, ".users[].name") }'
echo ""
echo "   All IDs:"
echo "$USERS" | $FK '{ print jpath($0, ".users.id") }'
echo ""

# ── jpath — extract into awk array ───────────────────────────────
echo "7) jpath() — extract into array and process:"
echo "$USERS" | $FK '{
    n = jpath($0, ".users", arr)
    printf "Found %d users\n", n
    n = jpath($0, ".users[].name", names)
    for (i = 1; i <= n; i++) printf "  %d. %s\n", i, names[i]
}'
echo ""

# ── Mixed: JSON + computation ────────────────────────────────────
echo "8) JSON + computation — API-style processing:"
EVENTS=$(cat <<'EOF'
{"event":"login","user":"alice","ts":1739700000}
{"event":"purchase","user":"alice","amount":29.99,"ts":1739700060}
{"event":"login","user":"bob","ts":1739700120}
{"event":"purchase","user":"bob","amount":14.50,"ts":1739700180}
{"event":"purchase","user":"alice","amount":45.00,"ts":1739700240}
EOF
)
echo "$EVENTS" | $FK -i json '
    $1 == "purchase" { spent[$2] += $3; orders[$2]++ }
    END {
        for (user in spent)
            printf "  %-8s %d orders  $%.2f total\n", user, orders[user], spent[user]
    }
'
