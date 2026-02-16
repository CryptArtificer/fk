#!/usr/bin/env bash
# 04-fk-features.sh — Showcase fk-only features
#
# Run: ./examples/04-fk-features.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"

echo "═══ fk-only features ═══"
echo ""

# ── Exponentiation (**) ──────────────────────────────────────────
echo "1) Exponentiation operator (**):"
printf "2\n3\n4\n5\n" | $FK '{ printf "%d ** 3 = %d\n", $1, $1 ** 3 }'
echo ""

echo "   Square roots via ** 0.5:"
printf "16\n25\n144\n" | $FK '{ printf "sqrt(%d) = %g\n", $1, $1 ** 0.5 }'
echo ""

# ── Hex literals ─────────────────────────────────────────────────
echo "2) Hex literals:"
echo "x" | $FK '{ printf "0xFF = %d, 0x1F = %d, 0xCAFE = %d\n", 0xFF, 0x1F, 0xCAFE }'
echo ""

# ── Unicode escapes ──────────────────────────────────────────────
echo "3) Unicode escape sequences:"
echo "x" | $FK 'BEGIN { printf "\\u2192 is: \u2192\n\\u2764 is: \u2764\n\\u00e9 is: \u00e9\n" }'
echo ""

# ── Negative field indexes ───────────────────────────────────────
echo "4) Negative field indexes — access from the end:"
echo "alpha beta gamma delta epsilon" | $FK '{
    print "Last field ($-1):", $-1
    print "Second-to-last ($-2):", $-2
    print "All but reordered:", $-1, $-2, $-3
}'
echo ""

# ── Computed field access $(expr) ────────────────────────────────
echo "5) Computed field access:"
echo "10 20 30 40 50" | $FK '{
    for (i = 1; i <= NF; i++) {
        printf "$(NF - %d + 1) = %s\n", i, $(NF - i + 1)
    }
}'
echo ""

# ── Time functions ───────────────────────────────────────────────
echo "6) Time functions:"
echo "x" | $FK '{
    now = systime()
    print "Epoch:", now
    print "Formatted:", strftime("%Y-%m-%d %H:%M:%S", now)
}'
echo ""

echo "   Convert date string to epoch and back:"
echo "x" | $FK '{
    epoch = mktime("2025 02 16 12 30 00")
    print "mktime(\"2025 02 16 12 30 00\") =", epoch
    print "strftime =>", strftime("%A, %B %d %Y at %H:%M", epoch)
}'
echo ""

# ── system() ─────────────────────────────────────────────────────
echo "7) system() — run external commands:"
echo "x" | $FK '{ status = system("echo \"  Hello from system()\""); print "  Exit status:", status }'
echo ""

# ── /dev/stderr ──────────────────────────────────────────────────
echo "8) /dev/stderr — separate output streams:"
echo "data" | $FK '{ print "stdout: " $0; print "stderr: " $0 > "/dev/stderr" }' 2>/tmp/fk_stderr_demo
echo "   (stderr went to /tmp/fk_stderr_demo: $(cat /tmp/fk_stderr_demo))"
rm -f /tmp/fk_stderr_demo
echo ""

# ── Unicode-aware string functions ───────────────────────────────
echo "9) Unicode-aware string operations:"
echo "café résumé naïve" | $FK '{
    for (i = 1; i <= NF; i++) {
        printf "  %-10s length=%d  substr(1,3)=%s\n", $i, length($i), substr($i, 1, 3)
    }
}'
echo ""

# ── nextfile ─────────────────────────────────────────────────────
echo "10) nextfile — skip to next input source:"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
printf "a\nb\nc\n" > "$TMPDIR/f1.txt"
printf "d\ne\nf\n" > "$TMPDIR/f2.txt"
echo "    First line of each file:"
$FK '{ print "  ", FILENAME, $0; nextfile }' "$TMPDIR/f1.txt" "$TMPDIR/f2.txt" 2>/dev/null || \
$FK 'FNR == 1 { print "  ", $0 }' "$TMPDIR/f1.txt" "$TMPDIR/f2.txt"
