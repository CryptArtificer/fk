#!/usr/bin/env bash
# 06-phase8-features.sh — Phase 8: signature features & extended builtins
#
# Run: ./examples/06-phase8-features.sh
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"

echo "═══ Phase 8: Signature features ═══"
echo ""

# ── Header names as field accessors ─────────────────────────────
echo "1) Header names as field accessors (-H mode):"
printf "name,age,city\nAlice,30,NYC\nBob,25,LA\nCarol,35,Chicago\n" | \
    show $FK -F, -H '$age > 28 { print "  ", $name, "age", $age, "from", $city }'
echo ""

echo "1b) Quoted column names (for special characters):"
printf "user-name,total.score\nAlice,95\nBob,87\n" | \
    show $FK -F, -H '{ print "  ", $"user-name", "scored", $"total.score" }'
echo ""

# ── match() with capture groups ─────────────────────────────────
echo "2) match() with capture groups:"
printf "2025-01-15\n2024-06-30\n" | show $FK '{
    match($0, "([0-9]{4})-([0-9]{2})-([0-9]{2})", cap)
    printf "  year=%s month=%s day=%s\n", cap[1], cap[2], cap[3]
}'
echo ""

# ── asort / asorti ──────────────────────────────────────────────
echo "3) asort — sort array by values:"
printf "banana\napple\ncherry\ndate\n" | show $FK '
    { a[NR] = $0 }
    END { asort(a); print "  Sorted:", join(a, " ") }
'
echo ""

echo "   asorti — sort by keys:"
printf "c:3\na:1\nb:2\n" | show $FK -F: '
    { a[$1] = $2 }
    END { n = asorti(a); print "  Keys:", join(a, " ") }
'
echo ""

# ── join() ──────────────────────────────────────────────────────
echo "4) join() — concatenate array values:"
echo "x" | show $FK 'BEGIN { a[1]="hello"; a[2]="beautiful"; a[3]="world"; print "  " join(a, " ") }'
echo "   join(a) with no separator uses OFS:"
echo "x" | show $FK -v 'OFS=|' 'BEGIN { a[1]="A"; a[2]="B"; a[3]="C"; print "  " join(a) }'
echo ""

# ── typeof() ────────────────────────────────────────────────────
echo "5) typeof() — runtime type introspection:"
echo "x" | show $FK 'BEGIN {
    n = 42; s = "hello"; a[1] = 1
    print "  42      →", typeof(n)
    print "  \"hello\" →", typeof(s)
    print "  array   →", typeof(a)
    print "  unknown →", typeof(missing)
}'
echo ""

# ── Bitwise operations ──────────────────────────────────────────
echo "6) Bitwise operations:"
echo "x" | show $FK 'BEGIN {
    printf "  and(0xFF, 0x0F) = %d\n", and(0xFF, 0x0F)
    printf "  or(0xF0, 0x0F)  = %d\n", or(0xF0, 0x0F)
    printf "  xor(0xFF, 0x0F) = %d\n", xor(0xFF, 0x0F)
    printf "  lshift(1, 8)    = %d\n", lshift(1, 8)
    printf "  rshift(256, 4)  = %d\n", rshift(256, 4)
}'
echo ""

# ── Math builtins ───────────────────────────────────────────────
echo "7) Extended math builtins:"
echo "x" | show $FK 'BEGIN {
    srand(42)
    printf "  rand()     = %.6f\n", rand()
    printf "  abs(-7.5)  = %g\n", abs(-7.5)
    printf "  ceil(2.3)  = %g\n", ceil(2.3)
    printf "  floor(2.7) = %g\n", floor(2.7)
    printf "  round(2.5) = %g\n", round(2.5)
    printf "  min(3, 7)  = %g\n", min(3, 7)
    printf "  max(3, 7)  = %g\n", max(3, 7)
    printf "  log2(8)    = %g\n", log2(8)
    printf "  log10(100) = %g\n", log10(100)
}'
echo ""

# ── String builtins ─────────────────────────────────────────────
echo "8) Extended string builtins:"
echo "x" | show $FK 'BEGIN {
    print "  trim(\"  hi  \")       =", "\"" trim("  hi  ") "\""
    print "  startswith(\"abc\",\"ab\") =", startswith("abc", "ab")
    print "  endswith(\"abc\",\"bc\")   =", endswith("abc", "bc")
    print "  repeat(\"ab\", 3)       =", repeat("ab", 3)
    print "  reverse(\"hello\")      =", reverse("hello")
    print "  chr(65)               =", chr(65)
    print "  ord(\"A\")              =", ord("A")
    print "  hex(255)              =", hex(255)
}'
echo ""

# ── parsedate ───────────────────────────────────────────────────
echo "9) parsedate — parse date strings to epoch:"
echo "x" | show $FK 'BEGIN {
    ts = parsedate("2025-02-16 14:30:00", "%Y-%m-%d %H:%M:%S")
    print "  2025-02-16 14:30:00 → epoch", ts
    print "  roundtrip:", strftime("%Y-%m-%d %H:%M:%S", ts)
}'
echo ""

echo "═══ Done! ═══"
