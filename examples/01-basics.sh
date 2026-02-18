#!/usr/bin/env bash
# 01-basics.sh — Field extraction, filtering, summing
#
# Run: ./examples/01-basics.sh
# Requires: fk in PATH or built at target/release/fk
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"

echo "═══ fk basics ═══"
echo ""

# ── Sample data ──────────────────────────────────────────────────
DATA=$(cat <<'EOF'
Alice 95 Math
Bob 87 English
Carol 92 Math
Dave 78 Science
Eve 96 English
Frank 64 Science
Grace 88 Math
Hank 73 English
EOF
)

# ── Print specific fields ────────────────────────────────────────
echo "1) Print name and score:"
echo "$DATA" | $FK '{ print $1, $2 }'
echo ""

# ── Filter rows ──────────────────────────────────────────────────
echo "2) Students scoring above 90:"
echo "$DATA" | $FK '$2 > 90 { print $1, $2 }'
echo ""

# ── Filter by pattern ────────────────────────────────────────────
echo "3) Math students:"
echo "$DATA" | $FK '/Math/ { print $1, $2 }'
echo ""

# ── Sum a column ─────────────────────────────────────────────────
echo "4) Total score:"
echo "$DATA" | $FK '{ sum += $2 } END { print "Total:", sum }'
echo ""

# ── Count records ────────────────────────────────────────────────
echo "5) Number of students:"
echo "$DATA" | $FK 'END { print "Count:", NR }'
echo ""

# ── Average (manual vs builtin) ──────────────────────────────────
echo "6) Average score:"
echo "$DATA" | $FK '{ sum += $2 } END { printf "Average: %.1f\n", sum / NR }'
echo "   (or with builtins):"
echo "$DATA" | $FK '{ a[NR]=$2 } END { printf "Average: %.1f\n", mean(a) }'
echo ""

# ── Min and max ──────────────────────────────────────────────────
echo "7) Highest and lowest score:"
echo "$DATA" | $FK '{ scores[NR]=$2; names[NR]=$1 }
    END {
        hi=1; lo=1
        for(i=2;i<=NR;i++) { if(scores[i]>scores[hi]) hi=i; if(scores[i]<scores[lo]) lo=i }
        print "High:", names[hi], scores[hi]; print "Low:", names[lo], scores[lo]
    }
'
echo ""

# ── Custom output separator ──────────────────────────────────────
echo "8) Tab-separated output:"
echo "$DATA" | $FK -v 'OFS=\t' '{ print $1, $2, $3 }'
echo ""

# ── Numbered output ──────────────────────────────────────────────
echo "9) Add line numbers:"
echo "$DATA" | $FK '{ printf "%2d. %s %s (%s)\n", NR, $1, $2, $3 }'
echo ""

# ── Full summary stats ──────────────────────────────────────────
echo "10) Summary stats (fk builtins):"
echo "$DATA" | $FK '{ a[NR]=$2 } END { printf "  n=%d mean=%.1f median=%.1f stddev=%.1f min=%s max=%s\n", length(a), mean(a), median(a), stddev(a), min(a), max(a) }'
