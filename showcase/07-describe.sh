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

section "4. --suggest: comprehensive tailored examples"
echo "The full tutorial — every example uses your actual column names and values:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}--suggest${C_RESET} ${C_FILE}orders.csv${C_RESET}\n\n"
$FK --suggest "$TMPDIR/orders.csv"

section "5. --describe: compressed files (transparent decompression)"
echo "Compressed files are decompressed on the fly (.gz, .zst, .bz2, .xz):"
echo ""
gzip -k "$TMPDIR/sales.csv"
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET} ${C_FILE}sales.csv.gz${C_RESET}\n\n"
$FK -d "$TMPDIR/sales.csv.gz"

section "6. Auto-detect input mode from extension"
echo "No need for -i csv — fk infers it from the file extension:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-H${C_RESET} ${C_YEL}'{ print \$region, \$revenue }'${C_RESET} ${C_FILE}sales.csv.gz${C_RESET}\n\n"
$FK -H '{ print $region, $revenue }' "$TMPDIR/sales.csv.gz" | $FK '{ printf "  %s\n", $0 }'

printf "\n${C_BOLD}Done.${C_RESET} --describe and --suggest turn unknown data into ready-to-run fk programs.\n"
