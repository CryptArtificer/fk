#!/usr/bin/env bash
# 17 — Describe and suggest: auto-detect format, infer schema, get programs
#
# Story: you have unfamiliar data files. Step 1: describe tells you what
# you're looking at. Step 2: suggest writes the fk programs for you.
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

strip_ansi() { sed $'s/\033\[[0-9;]*m//g'; }

section "1. Describe — instant schema from any file"

echo "Point fk at a CSV and get format, columns, types, and samples:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET} ${C_FILE}orders.csv${C_RESET}\n"
$FK -d "$TMPDIR/orders.csv"

echo "Works on JSON Lines too:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET} ${C_FILE}api.jsonl${C_RESET}\n"
$FK -d "$TMPDIR/api.jsonl"

echo "And piped data — format and headers inferred automatically:"
echo ""
printf "  ${C_DIM}\$${C_RESET} ${C_YEL}df -h | ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET}\n"
printf "Filesystem      Size  Used  Avail  Use%%\n/dev/sda1       100G   45G    55G   45%%\n/dev/sdb1       500G  320G   180G   64%%\ntmpfs           8.0G  1.2G   6.8G   15%%\n" | $FK -d

section "2. Compressed files — transparent decompression"

gzip -k "$TMPDIR/sales.csv"
echo "No special flags needed — fk decompresses .gz, .zst, .bz2, .xz on the fly:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}-d${C_RESET} ${C_FILE}sales.csv.gz${C_RESET}\n"
$FK -d "$TMPDIR/sales.csv.gz"

echo "Input mode is inferred from the extension (csv.gz → CSV):"
show $FK -H '{ printf "  %-6s %-8s $%s\n", $region, $product, $revenue }' "$TMPDIR/sales.csv.gz"

section "3. Suggest — fk writes the programs for you"

echo "Don't know where to start? --suggest analyzes your data and proposes programs:"
echo ""
printf "  ${C_DIM}\$ ${C_CYAN}${C_BOLD}fk${C_RESET} ${C_FLAG}--suggest${C_RESET} ${C_FILE}orders.csv${C_RESET}\n"

suggest_output=$($FK --suggest "$TMPDIR/orders.csv" 2>&1)
echo "$suggest_output"

section "4. Running every suggestion"

echo "Each command above was generated automatically. Let's run them all:"

cmds=$(echo "$suggest_output" | strip_ansi | grep '^ *fk ' | sed 's/^ *//')

while IFS= read -r cmd; do
    display="${cmd//$TMPDIR\//}"
    echo ""
    printf "  ${C_DIM}\$${C_RESET} ${C_CYAN}%s${C_RESET}\n" "$display"
    echo ""
    runcmd="${FK}${cmd#fk}"
    eval "$runcmd" 2>&1 | while IFS= read -r line; do
        printf "    %s\n" "$line"
    done
done <<< "$cmds"

section "5. Suggestions adapt to data shape"

echo "String + numeric CSV (sales data):"
echo ""
$FK --suggest "$TMPDIR/sales.csv" 2>&1

echo ""
echo "JSON Lines (API logs):"
echo ""
$FK --suggest "$TMPDIR/api.jsonl" 2>&1

printf "\n${C_BOLD}Done.${C_RESET} -d shows schema; --suggest writes programs. Works on any format.\n"
