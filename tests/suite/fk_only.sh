#!/usr/bin/env bash
# fk_only.sh — tests for fk-only features (no awk equivalent)
#
# Each test runs fk and checks against known expected output.
# Covers: D1-D30 (stats, arrays, string builtins, I/O, formats).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_runner.sh"
ensure_fk

# ── generate test data ──────────────────────────────────────────────

W="$TMPDIR_SUITE"
gen_numbers > "$W/numbers.txt"
gen_words   > "$W/words.txt"
gen_scores  > "$W/scores.txt"
gen_text    > "$W/text.txt"
gen_file1   > "$W/f1.txt"
gen_file2   > "$W/f2.txt"
gen_lookup  > "$W/lookup.txt"
gen_sparse  > "$W/sparse.txt"

# known numeric data for exact stats
printf "10\n20\n30\n40\n50\n" > "$W/nums5.txt"

# CSV with header
printf "name,dept,salary\nalice,eng,95000\nbob,sales,72000\ncarol,eng,105000\n" > "$W/hdr.csv"

# JSON Lines
printf '{"name":"alice","age":30}\n{"name":"bob","age":25}\n{"name":"carol","age":35}\n' > "$W/data.jsonl"

# config for match tests
printf "host=localhost\nport=8080\ndebug=true\ntimeout=30\n" > "$W/kv.txt"

# for slurp test
printf "line1\nline2\nline3\n" > "$W/slurp.txt"

# mixed log for regex var test
printf "error: disk full\ninfo: ok\nwarn: slow\nerror: timeout\ninfo: done\n" > "$W/mixlog.txt"

# two-column for inv test
printf "a 1\nb 2\nc 3\n" > "$W/pairs.txt"

# data with padding test
printf "alice 100\nbob 200\ncarol 300\n" > "$W/pad.txt"

# ════════════════════════════════════════════════════════════════════
section "D. Statistics"
# ════════════════════════════════════════════════════════════════════

# nums5: 10 20 30 40 50 → mean=30, median=30, stddev=√200≈14.14, var=200
out="$($FK '{a[NR]=$1} END{print median(a)}' "$W/nums5.txt")"
assert_eq "D1" "median" "$out" "30"

out="$($FK '{a[NR]=$1} END{print stddev(a)}' "$W/nums5.txt")"
assert_match "D2" "stddev" "$out" "^14\.1"

out="$($FK '{a[NR]=$1} END{print p(a,95)}' "$W/nums5.txt")"
assert_nonzero "D3" "percentile p95" "$out"

out="$($FK '{a[NR]=$1} END{print iqm(a)}' "$W/nums5.txt")"
assert_nonzero "D4" "interquartile mean" "$out"

out="$($FK '{a[NR]=$1} END{print variance(a)}' "$W/nums5.txt")"
assert_eq "D5" "variance" "$out" "200"

out="$($FK '{a[NR]=$1} END{printf "n=%d mean=%.0f\n", length(a), mean(a)}' "$W/nums5.txt")"
assert_eq "D6" "full summary" "$out" "n=5 mean=30"

# ════════════════════════════════════════════════════════════════════
section "D. Array operations"
# ════════════════════════════════════════════════════════════════════

# D7 shuffle: verify all lines present (sorted output = sorted input)
out="$($FK '{a[NR]=$0} END{shuf(a); print a}' "$W/nums5.txt" | sort -n)"
assert_eq "D7" "shuffle preserves values" "$out" "$(sort -n "$W/nums5.txt")"

# D8 sample: verify count
out="$($FK '{a[NR]=$0} END{samp(a,3); print a}' "$W/nums5.txt" | wc -l | tr -d ' ')"
assert_eq "D8" "sample count" "$out" "3"

# D9 seq
out="$($FK 'BEGIN{seq(a,1,5); print a}' < /dev/null)"
assert_eq "D9" "seq 1-5" "$out" "$(printf "1\n2\n3\n4\n5")"

# D10 uniq
out="$($FK '{a[NR]=$0} END{uniq(a); print a}' "$W/words.txt" | sort)"
expected="$(sort -u "$W/words.txt")"
assert_eq "D10" "uniq array" "$out" "$expected"

# D11 diff
out="$($FK 'NR==FNR{a[$0]++;next}{b[$0]++} END{diff(a,b); for(k in a) print k}' "$W/f1.txt" "$W/f2.txt" | sort)"
assert_eq "D11" "set difference" "$out" "$(printf "alpha\ngamma")"

# D12 inter
out="$($FK 'NR==FNR{a[$0]++;next}{b[$0]++} END{inter(a,b); for(k in a) print k}' "$W/f1.txt" "$W/f2.txt" | sort)"
assert_eq "D12" "set intersection" "$out" "$(printf "beta\ndelta")"

# D13 union
out="$($FK 'NR==FNR{a[$0]++;next}{b[$0]++} END{union(a,b); for(k in a) print k}' "$W/f1.txt" "$W/f2.txt" | sort)"
assert_eq "D13" "set union" "$out" "$(printf "alpha\nbeta\ndelta\nepsilon\ngamma\nzeta")"

# D14 inv
out="$($FK '{a[$1]=$2} END{inv(a); for(k in a) print k, a[k]}' "$W/pairs.txt" | sort)"
assert_eq "D14" "invert key/value" "$out" "$(printf "1 a\n2 b\n3 c")"

# D15 tidy (remove empties) — tidy removes empty-value keys; verify count
out="$($FK '{a[NR]=$0} END{tidy(a); print length(a)}' "$W/sparse.txt")"
assert_eq "D15" "tidy (remove empties)" "$out" "5"

# ════════════════════════════════════════════════════════════════════
section "D. String builtins"
# ════════════════════════════════════════════════════════════════════

# D16 trim
printf "  hello  \n  world  \n" > "$W/trimme.txt"
out="$($FK '{print trim($0)}' "$W/trimme.txt")"
assert_eq "D16" "trim" "$out" "$(printf "hello\nworld")"

# D17 lpad/rpad
out="$($FK '{print lpad($1,10), rpad($2,10)}' "$W/pairs.txt")"
expected="$(printf "         a 1         \n         b 2         \n         c 3         ")"
assert_eq "D17" "lpad/rpad" "$out" "$expected"

# D18 repeat
out="$($FK 'BEGIN{print repeat("ab",3)}' < /dev/null)"
assert_eq "D18" "repeat" "$out" "ababab"

# D19 reverse (unicode-aware)
out="$($FK '{print reverse($0)}' <<< "hello")"
assert_eq "D19" "reverse string" "$out" "olleh"

# ════════════════════════════════════════════════════════════════════
section "D. Input modes"
# ════════════════════════════════════════════════════════════════════

# D20 named column access (-H with CSV)
out="$($FK -i csv -H '{print $name, $salary}' "$W/hdr.csv")"
expected="$(printf "alice 95000\nbob 72000\ncarol 105000")"
assert_eq "D20" "named column access" "$out" "$expected"

# D21 CSV auto-detect from extension
out="$($FK -H '{print $name}' "$W/hdr.csv")"
assert_eq "D21" "CSV auto-detect" "$out" "$(printf "alice\nbob\ncarol")"

# D22 JSON Lines input
out="$($FK -i json '{print $1, $2}' "$W/data.jsonl")"
assert_nonzero "D22" "JSON Lines input" "$out"

# D23 slurp
out="$($FK "BEGIN{n=slurp(\"$W/slurp.txt\",w); print n}" < /dev/null)"
assert_eq "D23" "slurp line count" "$out" "3"

# D26 --describe
out="$($FK --describe "$W/hdr.csv" 2>&1)"
assert_nonzero "D26" "describe mode" "$out"

# ════════════════════════════════════════════════════════════════════
section "D. Pattern & expression features"
# ════════════════════════════════════════════════════════════════════

# D27 match with capture groups (string pattern — regex literals in match are a known gap)
out="$($FK '{if(match($0, "(\\w+)=(\\d+)", m)) print m[1], m[2]}' "$W/kv.txt")"
assert_eq "D27" "match capture groups" "$out" "$(printf "port 8080\ntimeout 30")"

# D28 bitwise operations
out="$($FK 'BEGIN{print and(0xFF, 0x0F), or(0xA0, 0x05), xor(0xFF, 0x0F)}' < /dev/null)"
assert_eq "D28" "bitwise ops" "$out" "15 165 240"

# D29 negative field indexing
out="$($FK '{print $-1, $-2}' "$W/pairs.txt")"
expected="$(printf "1 a\n2 b\n3 c")"
assert_eq "D29" "negative field index" "$out" "$expected"

# D30 computed regex from variable
out="$($FK -v pat="error|warn" '$0 ~ pat' "$W/mixlog.txt")"
expected="$(printf "error: disk full\nwarn: slow\nerror: timeout")"
assert_eq "D30" "computed regex" "$out" "$expected"

# ════════════════════════════════════════════════════════════════════
print_summary "fk_only"
