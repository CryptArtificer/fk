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

# ── Average ──────────────────────────────────────────────────────
echo "6) Average score:"
echo "$DATA" | $FK '{ sum += $2 } END { printf "Average: %.1f\n", sum / NR }'
echo ""

# ── Min and max ──────────────────────────────────────────────────
echo "7) Highest and lowest score:"
echo "$DATA" | $FK '
    NR == 1 { min = max = $2; min_name = max_name = $1 }
    $2 > max { max = $2; max_name = $1 }
    $2 < min { min = $2; min_name = $1 }
    END { print "High:", max_name, max; print "Low:", min_name, min }
'
echo ""

# ── Custom output separator ──────────────────────────────────────
echo "8) Tab-separated output:"
echo "$DATA" | $FK -v 'OFS=\t' '{ print $1, $2, $3 }'
echo ""

# ── Numbered output ──────────────────────────────────────────────
echo "9) Add line numbers:"
echo "$DATA" | $FK '{ printf "%2d. %s %s (%s)\n", NR, $1, $2, $3 }'
