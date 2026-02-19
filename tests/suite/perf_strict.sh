#!/usr/bin/env bash
# perf_strict.sh — performance benchmarks with warmup + median/p90
#
# Generates large data, times each tool multiple times, and reports median/p90.
# Default: 1M lines. Override with BENCH_LINES=N, WARMUP=N, REPS=N.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_runner.sh"
ensure_fk

export LC_ALL=C

AWK="${AWK:-awk}"
BENCH_LINES="${BENCH_LINES:-1000000}"
WARMUP="${WARMUP:-1}"
REPS="${REPS:-5}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/bench_data}"

mkdir -p "$OUT_DIR"
STAMP="$(date +%Y%m%d-%H%M%S)"
REPORT="$OUT_DIR/perf_strict_${BENCH_LINES}_${STAMP}.txt"

logf() {
    printf "$@" | tee -a "$REPORT"
}

log_section() {
    logf "\n${BOLD}━━ %s ━━${RESET}\n" "$1"
}

log_section "perf_strict: ${BENCH_LINES} lines, warmup=${WARMUP}, reps=${REPS}"

logf "  host: %s\n" "$(uname -a)"
if "$FK" --version >/dev/null 2>&1; then
    logf "  fk:   %s\n" "$("$FK" --version | tr -d '\r')"
fi
if "$AWK" --version >/dev/null 2>&1; then
    logf "  awk:  %s\n" "$("$AWK" --version 2>/dev/null | head -n 1 | tr -d '\r')"
elif "$AWK" -W version >/dev/null 2>&1; then
    logf "  awk:  %s\n" "$("$AWK" -W version 2>/dev/null | head -n 1 | tr -d '\r')"
fi

W="$TMPDIR_SUITE"

log_section "Generating ${BENCH_LINES}-line test data"

$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    for (i = 1; i <= n; i++)
        printf "%d %s %d %s\n", i, "user_" int(rand()*1000), int(rand()*10000), (rand()>0.5?"active":"inactive")
}' > "$W/large.txt"

logf "  %s lines in %s\n" "$(wc -l < "$W/large.txt" | tr -d ' ')" "$W/large.txt"

$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    for (i = 1; i <= n; i++)
        printf "%d,%s,%d,%s\n", i, "user_" int(rand()*1000), int(rand()*10000), (rand()>0.5?"active":"inactive")
}' > "$W/large.csv"

$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    split("alpha bravo charlie delta echo foxtrot golf hotel india juliet", w, " ")
    for (i = 1; i <= n; i++) print w[int(rand()*10)+1]
}' > "$W/words_large.txt"

$AWK -v n="$BENCH_LINES" 'BEGIN {
    srand(42)
    for (i = 1; i <= n; i++) printf "%.2f\n", rand()*1000
}' > "$W/nums_large.txt"

run_cmd_secs() {
    local cmd="$1"
    python3 - "$cmd" <<'PY'
import sys
import time
import subprocess
cmd = sys.argv[1]
start = time.perf_counter()
subprocess.run(cmd, shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
end = time.perf_counter()
print(f"{end - start:.6f}")
PY
}

stats() {
    python3 - "$@" <<'PY'
import sys
import statistics
import math
vals = [float(v) for v in sys.argv[1:]]
vals_sorted = sorted(vals)

def percentile(vals, p):
    if not vals:
        return float('nan')
    k = (len(vals) - 1) * p / 100.0
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return vals_sorted[int(k)]
    return vals_sorted[f] * (c - k) + vals_sorted[c] * (k - f)

med = statistics.median(vals_sorted)
p90 = percentile(vals_sorted, 90)
print(f"{med:.6f} {p90:.6f}")
PY
}

bench() {
    local id="$1" desc="$2" name1="$3" cmd1="$4" name2="$5" cmd2="$6"

    for _ in $(seq 1 "$WARMUP"); do
        run_cmd_secs "$cmd1" >/dev/null
        run_cmd_secs "$cmd2" >/dev/null
    done

    local t1=() t2=()
    for _ in $(seq 1 "$REPS"); do t1+=("$(run_cmd_secs "$cmd1")"); done
    for _ in $(seq 1 "$REPS"); do t2+=("$(run_cmd_secs "$cmd2")"); done

    local m1 p1 m2 p2 ratio
    read -r m1 p1 <<< "$(stats "${t1[@]}")"
    read -r m2 p2 <<< "$(stats "${t2[@]}")"
    ratio="$(python3 - "$m1" "$m2" <<'PY'
import sys
m1 = float(sys.argv[1])
m2 = float(sys.argv[2])
if m2 == 0:
    print("?")
else:
    print(f"{m1 / m2:.2f}")
PY
)"

    logf "  %-6s %-30s  %s med=%0.3fs p90=%0.3fs  %s med=%0.3fs p90=%0.3fs  ratio: %sx\n" \
        "$id" "$desc" "$name1" "$m1" "$p1" "$name2" "$m2" "$p2" "$ratio"
    ((_pass++)) || true
}

log_section "Performance: fk vs awk (${BENCH_LINES} lines)"

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
    "awk" "$AWK 'NR==FNR{a[\$2]=\$3;next} \$2 in a{print \$0,a[\$2]}' $W/large.txt $W/large.txt" \
    "fk"  "$FK  'NR==FNR{a[\$2]=\$3;next} \$2 in a{print \$0,a[\$2]}' $W/large.txt $W/large.txt"

log_section "Performance: fk vs tools (${BENCH_LINES} lines)"

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

log_section "Performance: fk-only features (${BENCH_LINES} lines)"

bench "B14" "mean() builtin" \
    "awk" "$AWK '{s+=\$1} END{print s/NR}' $W/nums_large.txt" \
    "fk"  "$FK  '{a[NR]=\$1} END{print mean(a)}' $W/nums_large.txt"

bench "B15" "median() builtin" \
    "sort" "sort -n $W/nums_large.txt | $AWK 'NR==c{print}' c=\$(wc -l < $W/nums_large.txt | $AWK '{print int(\$1/2)}')" \
    "fk"   "$FK  '{a[NR]=\$1} END{print median(a)}' $W/nums_large.txt"

bench "B16" "reverse() builtin" \
    "rev" "rev $W/words_large.txt" \
    "fk"  "$FK '{print reverse(\$0)}' $W/words_large.txt"

logf "\n${BOLD}━━ perf_strict summary ━━${RESET}\n"
logf "  %d benchmarks completed on %s lines\n" "$_pass" "$BENCH_LINES"
logf "  ${DIM}median/p90 from %s runs after %s warmup (ratio = tool_median / fk_median)${RESET}\n" "$REPS" "$WARMUP"
logf "  report: %s\n" "$REPORT"
