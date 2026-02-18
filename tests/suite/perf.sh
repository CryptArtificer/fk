#!/usr/bin/env bash
# perf.sh — performance benchmarks: fk vs awk vs native tools
#
# Generates large data, times each tool, reports wall-clock comparison.
# Default: 100k lines. Override with BENCH_LINES=N.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_runner.sh"
ensure_fk

AWK="${AWK:-awk}"
BENCH_LINES="${BENCH_LINES:-100000}"

W="$TMPDIR_SUITE"

# ── colors ──────────────────────────────────────────────────────────

CYAN=$'\033[36m'

# ── generate large data ─────────────────────────────────────────────

section "Generating ${BENCH_LINES}-line test data"

$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    for (i = 1; i <= n; i++)
        printf "%d %s %d %s\n", i, "user_" int(rand()*1000), int(rand()*10000), (rand()>0.5?"active":"inactive")
}' > "$W/large.txt"

printf "  %s lines in %s\n" "$(wc -l < "$W/large.txt" | tr -d ' ')" "$W/large.txt"

# CSV version
$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    for (i = 1; i <= n; i++)
        printf "%d,%s,%d,%s\n", i, "user_" int(rand()*1000), int(rand()*10000), (rand()>0.5?"active":"inactive")
}' > "$W/large.csv"

# words for frequency tests
$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    split("alpha bravo charlie delta echo foxtrot golf hotel india juliet", w, " ")
    for (i = 1; i <= n; i++) print w[int(rand()*10)+1]
}' > "$W/words_large.txt"

# numbers for stats
$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    for (i = 1; i <= n; i++) printf "%.2f\n", rand()*1000
}' > "$W/nums_large.txt"

# ── timing helper ───────────────────────────────────────────────────

# bench ID "description" "cmd1_name" "cmd1" "cmd2_name" "cmd2"
bench() {
    local id="$1" desc="$2"
    local name1="$3" cmd1="$4"
    local name2="$5" cmd2="$6"

    local t1 t2 ratio

    t1="$( { time eval "$cmd1" > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' )"
    t2="$( { time eval "$cmd2" > /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' )"

    # extract seconds (handle 0m0.123s format)
    local s1 s2
    s1="$(echo "$t1" | sed 's/[^0-9.]//g; s/^0*//' )"
    s2="$(echo "$t2" | sed 's/[^0-9.]//g; s/^0*//' )"

    # avoid division by zero
    if command -v bc &>/dev/null && [[ -n "$s2" ]] && [[ "$s2" != "0" ]]; then
        ratio="$(echo "scale=1; $s1 / $s2" | bc 2>/dev/null)" || ratio="?"
    else
        ratio="?"
    fi

    printf "  %-6s %-35s  ${DIM}%-3s${RESET} %s  ${CYAN}%-3s${RESET} %s  ${DIM}ratio: %sx${RESET}\n" \
        "$id" "$desc" "$name1" "$t1" "$name2" "$t2" "$ratio"
    ((_pass++)) || true
}

# ════════════════════════════════════════════════════════════════════
section "Performance: fk vs awk ($BENCH_LINES lines)"
# ════════════════════════════════════════════════════════════════════

bench "B1" "print \$2" \
    "awk" "$AWK '{print \$2}' $W/large.txt" \
    "fk"  "$FK  '{print \$2}' $W/large.txt"

bench "B2" "sum column" \
    "awk" "$AWK '{s+=\$3} END{print s}' $W/large.txt" \
    "fk"  "$FK  '{s+=\$3} END{print s}' $W/large.txt"

bench "B3" "pattern match" \
    "awk" "$AWK '/active/{c++} END{print c}' $W/large.txt" \
    "fk"  "$FK  '/active/{c++} END{print c}' $W/large.txt"

bench "B4" "field arithmetic" \
    "awk" "$AWK '{\$5=\$1+\$3; print}' $W/large.txt" \
    "fk"  "$FK  '{\$5=\$1+\$3; print}' $W/large.txt"

bench "B5" "associative array" \
    "awk" "$AWK '{a[\$2]++} END{for(k in a) print k,a[k]}' $W/large.txt" \
    "fk"  "$FK  '{a[\$2]++} END{for(k in a) print k,a[k]}' $W/large.txt"

bench "B6" "frequency count" \
    "awk" "$AWK '{a[\$0]++} END{for(k in a) print a[k],k}' $W/words_large.txt" \
    "fk"  "$FK  '{a[\$0]++} END{for(k in a) print a[k],k}' $W/words_large.txt"

bench "B7" "gsub" \
    "awk" "$AWK '{gsub(/user/,\"USER\")}1' $W/large.txt" \
    "fk"  "$FK  '{gsub(/user/,\"USER\")}1' $W/large.txt"

bench "B8" "NR==FNR join" \
    "awk" "$AWK 'NR==FNR{a[\$2]=\$3;next} a[\$2]!=\"\"{print \$0,a[\$2]}' $W/large.txt $W/large.txt" \
    "fk"  "$FK  'NR==FNR{a[\$2]=\$3;next} a[\$2]!=\"\"{print \$0,a[\$2]}' $W/large.txt $W/large.txt"

# ════════════════════════════════════════════════════════════════════
section "Performance: fk vs tools ($BENCH_LINES lines)"
# ════════════════════════════════════════════════════════════════════

bench "B9" "wc -l" \
    "wc"  "wc -l < $W/large.txt" \
    "fk"  "$FK 'END{print NR}' $W/large.txt"

bench "B10" "grep pattern" \
    "grep" "grep active $W/large.txt" \
    "fk"   "$FK '/active/' $W/large.txt"

bench "B11" "cut -d' ' -f2" \
    "cut" "cut -d' ' -f2 $W/large.txt" \
    "fk"  "$FK '{print \$2}' $W/large.txt"

bench "B12" "head -100" \
    "head" "head -100 $W/large.txt" \
    "fk"   "$FK 'NR>100{exit}1' $W/large.txt"

bench "B13" "uniq (sorted input)" \
    "uniq" "sort $W/words_large.txt | uniq" \
    "fk"   "$FK '!seen[\$0]++' $W/words_large.txt"

# ════════════════════════════════════════════════════════════════════
section "Performance: fk-only features ($BENCH_LINES lines)"
# ════════════════════════════════════════════════════════════════════

bench "B14" "mean() builtin" \
    "awk" "$AWK '{s+=\$1} END{print s/NR}' $W/nums_large.txt" \
    "fk"  "$FK  '{a[NR]=\$1} END{print mean(a)}' $W/nums_large.txt"

bench "B15" "median() builtin" \
    "sort" "sort -n $W/nums_large.txt | $AWK 'NR==c{print}' c=\$(wc -l < $W/nums_large.txt | $AWK '{print int(\$1/2)}')" \
    "fk"   "$FK  '{a[NR]=\$1} END{print median(a)}' $W/nums_large.txt"

bench "B16" "reverse() builtin" \
    "rev" "rev $W/words_large.txt" \
    "fk"  "$FK '{print reverse(\$0)}' $W/words_large.txt"

# ════════════════════════════════════════════════════════════════════
printf "\n${BOLD}━━ perf summary ━━${RESET}\n"
printf "  %d benchmarks completed on %s lines\n" "$_pass" "$BENCH_LINES"
printf "  ${DIM}ratio = tool_time / fk_time (>1 means fk is faster)${RESET}\n"
