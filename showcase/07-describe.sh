#!/usr/bin/env bash
# 07 — Describe and suggest: auto-detect format, infer schema, generate programs
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. --describe: auto-detect format and schema"
echo "Feed fk an unknown CSV file — it detects format, headers, and column types:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}--describe${C_RESET} ${C_FILE}sales.csv${C_RESET}\n\n"
$FK --describe "$TMPDIR/sales.csv"

section "2. --describe: JSON Lines"
echo "Same for JSON Lines — auto-detected, keys become columns:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET} ${C_FILE}api.jsonl${C_RESET}\n\n"
$FK -d "$TMPDIR/api.jsonl"

section "3. --describe: whitespace-delimited data from stdin"
echo "Pipe anything in — format and header are inferred automatically:"
echo ""
printf "  ${C_DIM}\$${C_RESET} ${C_YEL}df -h | ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET}\n\n"
printf "Filesystem      Size  Used  Avail  Use%%\n/dev/sda1       100G   45G    55G   45%%\n/dev/sdb1       500G  320G   180G   64%%\ntmpfs           8.0G  1.2G   6.8G   15%%\n" | $FK -d

section "4. --suggest: tailored examples from schema"
echo "fk --suggest generates ~40 copy-pasteable commands using real column names"
echo "and values from the data. Here's the output (first 3 sections):"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}--suggest${C_RESET} ${C_FILE}orders.csv${C_RESET}\n\n"
$FK --suggest "$TMPDIR/orders.csv" 2>&1 | head -60
echo "  ..."
echo ""

section "5. Running the suggested commands"
echo "Let's run some of those suggestions on the actual data:"
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '{ s += \$amount } END { print s }' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET} $($FK -H -i csv '{ s += $amount } END { print s }' "$TMPDIR/orders.csv")"
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '\$customer ~ /Alice/' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET}"
$FK -H -i csv '$customer ~ /Alice/' "$TMPDIR/orders.csv" | $FK '{ printf "  %s\n", $0 }'
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '{ a[\$customer] += \$amount } END { for (k in a) print k, a[k] }' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET}"
$FK -H -i csv '{ a[$customer] += $amount } END { for (k in a) printf "  %-16s $%.2f\n", k, a[k] }' "$TMPDIR/orders.csv"
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '{ a[NR] = \$amount } END { printf \"mean=%.2f stddev=%.2f\\n\", mean(a), stddev(a) }' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET} $($FK -H -i csv '{ a[NR] = $amount } END { printf "mean=%.2f stddev=%.2f\n", mean(a), stddev(a) }' "$TMPDIR/orders.csv")"
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '{ a[NR] = \$amount } END { printf \"min=%.2f p25=%.2f median=%.2f p75=%.2f max=%.2f\\n\", min(a), p(a,25), median(a), p(a,75), max(a) }' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET} $($FK -H -i csv '{ a[NR] = $amount } END { printf "min=%.2f p25=%.2f median=%.2f p75=%.2f max=%.2f\n", min(a), p(a,25), median(a), p(a,75), max(a) }' "$TMPDIR/orders.csv")"
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '{ if (\$amount > 320) print \"high\", \$amount; else print \"low\", \$amount }' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET}"
$FK -H -i csv '{ if ($amount > 320) print "high", $amount; else print "low", $amount }' "$TMPDIR/orders.csv" | $FK '{ printf "  %s\n", $0 }'
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '!seen[\$customer]++' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET}"
$FK -H -i csv '!seen[$customer]++' "$TMPDIR/orders.csv" | $FK '{ printf "  %s\n", $0 }'
echo ""

echo "${C_DIM}Suggest said:${C_RESET} ${C_YEL}fk -H -i csv '{ n = split(\$customer, parts, \" \"); print parts[1] }' orders.csv${C_RESET}"
echo "${C_DIM}→${C_RESET}"
$FK -H -i csv '{ n = split($customer, parts, " "); print parts[1] }' "$TMPDIR/orders.csv" | $FK '{ printf "  %s\n", $0 }'
echo ""

section "6. --describe: compressed files (transparent decompression)"
echo "Compressed files are decompressed on the fly (.gz, .zst, .bz2, .xz):"
echo ""
gzip -k "$TMPDIR/sales.csv"
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET} ${C_FILE}sales.csv.gz${C_RESET}\n\n"
$FK -d "$TMPDIR/sales.csv.gz"

section "7. Auto-detect input mode from extension"
echo "No need for -i csv — fk infers it from the file extension:"
echo ""
show $FK -H '{ printf "  %-6s $%s\n", $region, $revenue }' "$TMPDIR/sales.csv.gz"

printf "\n${C_BOLD}Done.${C_RESET} --describe and --suggest turn unknown data into ready-to-run fk programs.\n"
