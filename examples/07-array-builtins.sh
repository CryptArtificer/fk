#!/usr/bin/env bash
# 07-array-builtins.sh — Lodash-inspired array operations
#
# Run: ./examples/07-array-builtins.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"

echo "═══ Array builtins ═══"
echo ""

# ── print arr ────────────────────────────────────────────────────
echo "1) print arr — smart array printing:"
echo "   Associative (prints sorted keys):"
printf 'banana\napple\ncherry\napple\n' | $FK '{ u[$1]++ } END { print u }'
echo "   Sequential (prints values in order):"
printf 'cherry\napple\nbanana\n' | $FK '{ a[NR]=$1 } END { asort(a); print a }'
echo ""

# ── keys() / vals() ─────────────────────────────────────────────
echo "2) keys() and vals():"
echo "x" | $FK 'BEGIN {
    a["name"]="Alice"; a["age"]="30"; a["city"]="NYC"
    print "  keys:", keys(a)
    print "  vals:", vals(a)
}'
echo ""

# ── uniq ─────────────────────────────────────────────────────────
echo "3) uniq() — deduplicate:"
printf 'red\nblue\nred\ngreen\nblue\nblue\n' | \
    $FK '{ a[NR]=$1 } END { n=uniq(a); print " ", n, "unique:"; print a }'
echo ""

# ── invert ───────────────────────────────────────────────────────
echo "4) inv() — swap keys and values:"
echo "x" | $FK 'BEGIN {
    a["US"]="United States"; a["UK"]="United Kingdom"; a["FR"]="France"
    inv(a)
    for (k in a) printf "  %s => %s\n", k, a[k]
}'
echo ""

# ── compact ──────────────────────────────────────────────────────
echo "5) tidy() — remove falsy entries:"
echo "x" | $FK 'BEGIN {
    a[1]="hello"; a[2]=""; a[3]=0; a[4]="world"; a[5]=""
    printf "  before: %d entries\n", length(a)
    tidy(a)
    printf "  after:  %d entries\n", length(a)
    for (k in a) printf "  a[%s] = %s\n", k, a[k]
}'
echo ""

# ── diff / inter / union ────────────────────────────────────────
echo "6) Set operations (a={apple,banana,cherry,date} b={banana,date,elderberry}):"
echo "   diff(a, b) — in a but not b:"
echo "x" | $FK 'BEGIN {
    a["apple"]=1; a["banana"]=1; a["cherry"]=1; a["date"]=1
    b["banana"]=1; b["date"]=1; b["elderberry"]=1
    diff(a, b); asorti(a); print "  ", join(a, " ")
}'

echo "   inter(a, b) — in both:"
echo "x" | $FK 'BEGIN {
    a["apple"]=1; a["banana"]=1; a["cherry"]=1; a["date"]=1
    b["banana"]=1; b["date"]=1; b["elderberry"]=1
    inter(a, b); asorti(a); print "  ", join(a, " ")
}'

echo "   union(a, b) — in either:"
echo "x" | $FK 'BEGIN {
    a["apple"]=1; a["banana"]=1
    b["cherry"]=1; b["date"]=1
    union(a, b); asorti(a); print "  ", join(a, " ")
}'
echo ""

# ── seq ──────────────────────────────────────────────────────────
echo "7) seq() — generate sequences:"
echo "x" | $FK 'BEGIN { seq(a, 1, 10); print "  1..10:", join(a, ",") }'
echo "x" | $FK 'BEGIN { seq(a, 5, -5); print "  5..-5:", join(a, ",") }'
echo ""

# ── shuffle / sample ────────────────────────────────────────────
echo "8) shuf() and samp():"
echo "x" | $FK 'BEGIN {
    srand(42)
    seq(deck, 1, 10)
    shuf(deck)
    print "  shuffled:", join(deck, ",")
    samp(deck, 3)
    print "  sampled 3:", join(deck, ",")
}'
echo ""

# ── slurp ────────────────────────────────────────────────────────
echo "9) slurp() — read file into string or array:"
echo "x" | $FK 'BEGIN {
    n = slurp("/etc/shells", lines)
    printf "  %d lines in /etc/shells, first: %s\n", n, lines[1]
}'
echo ""

# ── lpad / rpad ──────────────────────────────────────────────────
echo "10) lpad() / rpad() — padding:"
printf 'Alice 95\nBob 87\nCharlie 100\n' | $FK '{
    printf "  |%s|%s|\n", rpad($1, 10), lpad($2, 5, "0")
}'
echo ""

# ── Real-world combo ─────────────────────────────────────────────
echo "11) Combo — unique sorted users from /etc/passwd:"
$FK -F: '!/^#/{ u[$1]++ } END { print u }' /etc/passwd | head -5
echo "  ... (showing first 5)"
echo ""

echo "═══ Done! ═══"
