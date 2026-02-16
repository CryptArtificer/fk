#!/usr/bin/env bash
# 02-text-transforms.sh — CSV wrangling, log parsing, frequency counting
#
# Run: ./examples/02-text-transforms.sh
set -euo pipefail
FK="${FK:-$(dirname "$0")/../target/release/fk}"

echo "═══ text transforms ═══"
echo ""

# ── CSV field extraction ─────────────────────────────────────────
echo "1) Extract columns from CSV:"
CSV=$(cat <<'EOF'
name,department,salary
Alice,Engineering,95000
Bob,Marketing,72000
Carol,Engineering,102000
Dave,Marketing,68000
Eve,Engineering,88000
EOF
)
echo "$CSV" | $FK -F, 'NR > 1 { print $1, $3 }'
echo ""

# ── Log parsing ──────────────────────────────────────────────────
echo "2) Parse log levels and count errors:"
LOGS=$(cat <<'EOF'
2025-01-15 10:23:01 INFO  Server started on port 8080
2025-01-15 10:23:05 WARN  Slow query detected (320ms)
2025-01-15 10:24:12 ERROR Connection timeout to db-replica-3
2025-01-15 10:24:15 INFO  Retrying connection...
2025-01-15 10:24:18 ERROR Connection refused by db-replica-3
2025-01-15 10:25:01 INFO  Failover to db-replica-4 complete
2025-01-15 10:25:30 WARN  Memory usage above 80%
2025-01-15 10:26:45 ERROR Disk space below 5% on /var/log
EOF
)
echo "$LOGS" | $FK '/ERROR/ { print $1, $2, substr($0, index($0, "ERROR") + 6) }'
echo ""

# ── Frequency count ──────────────────────────────────────────────
echo "3) Count log levels:"
echo "$LOGS" | $FK '
    { count[$3]++ }
    END { for (level in count) print level, count[level] }
'
echo ""

# ── Field reordering ─────────────────────────────────────────────
echo "4) Reorder and reformat CSV → TSV:"
echo "$CSV" | $FK -F, -v 'OFS=\t' 'NR > 1 { print $2, $1, $3 }'
echo ""

# ── Deduplication ────────────────────────────────────────────────
echo "5) Unique departments from CSV:"
echo "$CSV" | $FK -F, 'NR > 1 && !seen[$2]++ { print $2 }'
echo ""

# ── Word frequency ───────────────────────────────────────────────
echo "6) Word frequency in text:"
TEXT="the quick brown fox jumps over the lazy dog the fox the dog"
echo "$TEXT" | $FK '{
    n = split($0, words, " ")
    for (i = 1; i <= n; i++) freq[words[i]]++
}
END { for (w in freq) printf "%3d %s\n", freq[w], w }
'
echo ""

# ── Running total ────────────────────────────────────────────────
echo "7) Running total:"
printf "10\n25\n-5\n30\n15\n" | $FK '{ sum += $1; printf "%4d  (total: %d)\n", $1, sum }'
