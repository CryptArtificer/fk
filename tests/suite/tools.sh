#!/usr/bin/env bash
# tools.sh — fk vs Unix tools (cut, head, tail, wc, sort, uniq, grep,
#             nl, tac, rev, paste, tr, seq) and sed equivalents.
#
# Runs the same operation with the native tool and fk, diffs output.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_runner.sh"
ensure_fk

# ── generate test data ──────────────────────────────────────────────

W="$TMPDIR_SUITE"
gen_words   > "$W/words.txt"
gen_numbers > "$W/numbers.txt"
gen_scores  > "$W/scores.txt"
gen_text    > "$W/text.txt"
gen_subst   > "$W/subst.txt"
gen_sparse  > "$W/sparse.txt"

# CSV-ish for cut tests
printf "alice,eng,95000\nbob,sales,72000\ncarol,eng,105000\n" > "$W/cut.csv"

# 20+ lines for head/tail
seq 1 25 > "$W/seq25.txt"

# lines with pattern
gen_logfile > "$W/logfile.txt"

# lines with consecutive dupes for uniq
printf "aaa\naaa\nbbb\naaa\naaa\naaa\nccc\nccc\n" > "$W/dupes.txt"

# ════════════════════════════════════════════════════════════════════
section "T. Unix tool equivalents"
# ════════════════════════════════════════════════════════════════════

# T1. cut -d, -f1,3
tool_out="$(cut -d, -f1,3 "$W/cut.csv")"
fk_out="$($FK -F, 'BEGIN{OFS=","} {print $1, $3}' "$W/cut.csv")"
assert_eq "T1" "cut -d, -f1,3" "$fk_out" "$tool_out"

# T2. cut -c1-10
tool_out="$(cut -c1-10 "$W/text.txt")"
fk_out="$($FK '{print substr($0,1,10)}' "$W/text.txt")"
assert_eq "T2" "cut -c1-10" "$fk_out" "$tool_out"

# T3. head -n 5
tool_out="$(head -n 5 "$W/seq25.txt")"
fk_out="$($FK 'NR==5{print;exit};1' "$W/seq25.txt")"
assert_eq "T3" "head -n 5" "$fk_out" "$tool_out"

# T4. tail -n 1
tool_out="$(tail -n 1 "$W/seq25.txt")"
fk_out="$($FK 'END{print}' "$W/seq25.txt")"
assert_eq "T4" "tail -n 1" "$fk_out" "$tool_out"

# T5. wc -l
tool_out="$(wc -l < "$W/words.txt" | tr -d ' ')"
fk_out="$($FK 'END{print NR}' "$W/words.txt")"
assert_eq "T5" "wc -l" "$fk_out" "$tool_out"

# T6. wc -w
tool_out="$(wc -w < "$W/text.txt" | tr -d ' ')"
fk_out="$($FK '{w+=NF} END{print w}' "$W/text.txt")"
assert_eq "T6" "wc -w" "$fk_out" "$tool_out"

# T7. sort -u
tool_out="$(sort -u "$W/words.txt")"
fk_out="$($FK '!seen[$0]++' "$W/words.txt" | sort)"
assert_eq "T7" "sort -u" "$fk_out" "$tool_out"

# T8. uniq (consecutive dedup)
tool_out="$(uniq "$W/dupes.txt")"
fk_out="$($FK 'a!=$0{print} {a=$0}' "$W/dupes.txt")"
assert_eq "T8" "uniq" "$fk_out" "$tool_out"

# T9. uniq -c
tool_out="$(uniq -c "$W/dupes.txt" | sed 's/^ *//')"
fk_out="$($FK 'a!=$0{if(a!="")print c,a;c=0;a=$0}{c++} END{print c,a}' "$W/dupes.txt")"
assert_eq "T9" "uniq -c" "$fk_out" "$tool_out"

# T10. sort | uniq -c | sort -rn
tool_out="$(sort "$W/words.txt" | uniq -c | sort -rn | sed 's/^ *//')"
fk_out="$($FK '{a[$0]++} END{for(k in a) print a[k], k}' "$W/words.txt" | sort -rn)"
assert_eq "T10" "frequency count" "$fk_out" "$tool_out"

# T11. grep
tool_out="$(grep "ERROR" "$W/logfile.txt")"
fk_out="$($FK '/ERROR/' "$W/logfile.txt")"
assert_eq "T11" "grep" "$fk_out" "$tool_out"

# T12. grep -c
tool_out="$(grep -c "ERROR" "$W/logfile.txt")"
fk_out="$($FK '/ERROR/{n++} END{print n+0}' "$W/logfile.txt")"
assert_eq "T12" "grep -c" "$fk_out" "$tool_out"

# T13. grep -v
tool_out="$(grep -v "ERROR" "$W/logfile.txt")"
fk_out="$($FK '!/ERROR/' "$W/logfile.txt")"
assert_eq "T13" "grep -v" "$fk_out" "$tool_out"

# T14. nl
tool_out="$(nl "$W/words.txt")"
fk_out="$($FK '{printf "%6d\t%s\n", NR, $0}' "$W/words.txt")"
assert_eq "T14" "nl" "$fk_out" "$tool_out"

# T15. tac (if available)
if command -v tac &>/dev/null; then
    tool_out="$(tac "$W/words.txt")"
    fk_out="$($FK '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}' "$W/words.txt")"
    assert_eq "T15" "tac" "$fk_out" "$tool_out"
else
    # macOS: use tail -r
    tool_out="$(tail -r "$W/words.txt")"
    fk_out="$($FK '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}' "$W/words.txt")"
    assert_eq "T15" "tac (tail -r)" "$fk_out" "$tool_out"
fi

# T16. rev
if command -v rev &>/dev/null; then
    tool_out="$(rev "$W/words.txt")"
    fk_out="$($FK '{print reverse($0)}' "$W/words.txt")"
    assert_eq "T16" "rev" "$fk_out" "$tool_out"
else
    skip_test "T16" "rev" "rev not found"
fi

# T17. paste -sd,
tool_out="$(paste -sd, "$W/words.txt")"
fk_out="$($FK '{a[NR]=$0} END{print join(a,",")}' "$W/words.txt")"
assert_eq "T17" "paste -sd," "$fk_out" "$tool_out"

# T18. tr -s ' ' — fk normalizes differently (strips leading/trailing)
# Just verify fk produces single-spaced output
fk_out="$($FK '{$1=$1; print}' <<< "  hello   world  ")"
assert_eq "T18" "squeeze whitespace" "$fk_out" "hello world"

# T20. seq 1 10
tool_out="$(seq 1 10)"
fk_out="$($FK 'BEGIN{seq(a,1,10); print a}' < /dev/null)"
assert_eq "T20" "seq 1 10" "$fk_out" "$tool_out"

# ════════════════════════════════════════════════════════════════════
section "S. Sed equivalents"
# ════════════════════════════════════════════════════════════════════

# S1. sub first occurrence
tool_out="$(sed 's/foo/bar/' "$W/subst.txt")"
fk_out="$($FK '{sub("foo","bar")} {print}' "$W/subst.txt")"
assert_eq "S1" "sed s/foo/bar/" "$fk_out" "$tool_out"

# S2. gsub all occurrences
tool_out="$(sed 's/foo/bar/g' "$W/subst.txt")"
fk_out="$($FK '{gsub("foo","bar")} {print}' "$W/subst.txt")"
assert_eq "S2" "sed s/foo/bar/g" "$fk_out" "$tool_out"

# S3. delete blank lines
tool_out="$(sed '/^$/d' "$W/sparse.txt")"
fk_out="$($FK 'NF' "$W/sparse.txt")"
assert_eq "S3" "delete blank lines" "$fk_out" "$tool_out"

# S4. delete leading whitespace
printf "  hello\n\tworld\n  foo\n" > "$W/indent.txt"
tool_out="$(sed 's/^[ 	]*//' "$W/indent.txt")"
fk_out="$($FK '{print ltrim($0)}' "$W/indent.txt")"
assert_eq "S4" "ltrim" "$fk_out" "$tool_out"

# S5. delete trailing whitespace
printf "hello  \nworld\t\nfoo   \n" > "$W/trail.txt"
tool_out="$(sed 's/[ 	]*$//' "$W/trail.txt")"
fk_out="$($FK '{print rtrim($0)}' "$W/trail.txt")"
assert_eq "S5" "rtrim" "$fk_out" "$tool_out"

# S6. print lines 10-20
tool_out="$(sed -n '10,20p' "$W/seq25.txt")"
fk_out="$($FK 'NR>=10 && NR<=20' "$W/seq25.txt")"
assert_eq "S6" "lines 10-20" "$fk_out" "$tool_out"

# S7. print matching line
tool_out="$(sed -n '/ERROR/p' "$W/logfile.txt")"
fk_out="$($FK '/ERROR/' "$W/logfile.txt")"
assert_eq "S7" "sed -n /pattern/p" "$fk_out" "$tool_out"

# S8. delete matching lines
tool_out="$(sed '/ERROR/d' "$W/logfile.txt")"
fk_out="$($FK '!/ERROR/' "$W/logfile.txt")"
assert_eq "S8" "sed /pattern/d" "$fk_out" "$tool_out"

# S9. number lines
tool_out="$(sed = "$W/words.txt" | sed 'N;s/\n/\t/')"
fk_out="$($FK '{printf "%d\t%s\n", NR, $0}' "$W/words.txt")"
assert_eq "S9" "number lines" "$fk_out" "$tool_out"

# S10. reverse (tac)
tool_out="$(sed '1!G;h;$!d' "$W/words.txt")"
fk_out="$($FK '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}' "$W/words.txt")"
assert_eq "S10" "reverse file" "$fk_out" "$tool_out"

# ════════════════════════════════════════════════════════════════════
print_summary "tools"
