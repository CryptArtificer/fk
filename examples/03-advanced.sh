#!/usr/bin/env bash
# 03-advanced.sh — User-defined functions, associative arrays, multi-file
#
# Run: ./examples/03-advanced.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "═══ advanced features ═══"
echo ""

# ── User-defined functions ───────────────────────────────────────
echo "1) Functions — letter grade conversion:"
SCORES="Alice 95\nBob 87\nCarol 72\nDave 64\nEve 58"
printf "$SCORES" | $FK '
function grade(score) {
    if (score >= 90) return "A"
    if (score >= 80) return "B"
    if (score >= 70) return "C"
    if (score >= 60) return "D"
    return "F"
}
{ printf "%-8s %3d  %s\n", $1, $2, grade($2) }
'
echo ""

# ── Recursive functions ──────────────────────────────────────────
echo "2) Recursive factorial:"
echo "10" | $FK '
function factorial(n) {
    if (n <= 1) return 1
    return n * factorial(n - 1)
}
{ printf "%d! = %d\n", $1, factorial($1) }
'
echo ""

# ── Associative arrays — group by ────────────────────────────────
echo "3) Group-by aggregation:"
DATA=$(cat <<'EOF'
Engineering Alice 95000
Marketing Bob 72000
Engineering Carol 102000
Marketing Dave 68000
Engineering Eve 88000
Sales Frank 55000
Sales Grace 61000
EOF
)
echo "$DATA" | $FK '
{
    dept_sum[$1] += $3
    dept_count[$1]++
}
END {
    for (dept in dept_sum) {
        avg = dept_sum[dept] / dept_count[dept]
        printf "%-15s %d staff  avg $%.0f\n", dept, dept_count[dept], avg
    }
}
'
echo ""

# ── Multi-file processing ────────────────────────────────────────
echo "4) Multi-file: compare two datasets:"

cat > "$TMPDIR/jan.txt" <<'EOF'
Alice 4200
Bob 3800
Carol 4500
EOF

cat > "$TMPDIR/feb.txt" <<'EOF'
Alice 4600
Bob 3900
Carol 4100
EOF

echo "January:"
$FK '{ print "  ", $1, $2 }' "$TMPDIR/jan.txt"
echo "February:"
$FK '{ print "  ", $1, $2 }' "$TMPDIR/feb.txt"
echo "Changes:"
$FK '
    NR == FNR { jan[$1] = $2; next }
    {
        diff = $2 - jan[$1]
        sign = diff >= 0 ? "+" : ""
        printf "  %-8s %s%d\n", $1, sign, diff
    }
' "$TMPDIR/jan.txt" "$TMPDIR/feb.txt" 2>/dev/null || \
$FK '{ jan[$1] = $2 }' "$TMPDIR/jan.txt" && \
$FK -v "data=$(cat "$TMPDIR/jan.txt" | $FK '{ printf "%s:%s,", $1, $2 }')" '
BEGIN {
    n = split(data, pairs, ",")
    for (i = 1; i <= n; i++) {
        if (pairs[i] != "") {
            split(pairs[i], kv, ":")
            jan[kv[1]] = kv[2]
        }
    }
}
{
    diff = $2 - jan[$1]
    sign = diff >= 0 ? "+" : ""
    printf "  %-8s %s%d\n", $1, sign, diff
}
' "$TMPDIR/feb.txt"
echo ""

# ── Pattern ranges ───────────────────────────────────────────────
echo "5) Pattern ranges — extract a section:"
CONFIG=$(cat <<'EOF'
[general]
name = myapp
version = 2.1

[database]
host = localhost
port = 5432
name = mydb

[cache]
host = redis.local
ttl = 300
EOF
)
echo "$CONFIG" | $FK '/\[database\]/,/^\[/ { if ($0 !~ /^\[/) print "  " $0 }'
echo ""

# ── Output to multiple files ─────────────────────────────────────
echo "6) Split input into separate files by key:"
RECORDS="A 10\nB 20\nA 30\nC 40\nB 50\nA 60"
printf "$RECORDS" | $FK -v "dir=$TMPDIR" '{ print $2 >> (dir "/" $1 ".txt") }'
for f in "$TMPDIR"/A.txt "$TMPDIR"/B.txt "$TMPDIR"/C.txt; do
    echo "  $(basename "$f"): $(cat "$f" | tr '\n' ' ')"
done
