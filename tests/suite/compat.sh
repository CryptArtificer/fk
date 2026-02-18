#!/usr/bin/env bash
# compat.sh — awk vs fk compatibility tests
#
# Runs the same program in both awk and fk, diffs output.
# Covers: original 100 programs (identical-output subset),
#         Pement one-liners (P1-P58), two-file idioms (C1-C5).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_runner.sh"
ensure_fk

AWK="${AWK:-awk}"

# helper: run same program in awk and fk, compare output
compat() {
    local id="$1" desc="$2" prog="$3"
    shift 3
    local awk_out fk_out
    awk_out="$($AWK "$prog" "$@" 2>/dev/null)" || true
    fk_out="$($FK  "$prog" "$@" 2>/dev/null)" || true
    assert_eq "$id" "$desc" "$fk_out" "$awk_out"
}

# same but with extra flags before program
compat_flags() {
    local id="$1" desc="$2"
    shift 2
    local args=()
    while [[ "$1" != "--" ]]; do args+=("$1"); shift; done
    shift  # consume --
    local prog="$1"; shift
    local awk_out fk_out
    awk_out="$($AWK "${args[@]}" "$prog" "$@" 2>/dev/null)" || true
    fk_out="$($FK  "${args[@]}" "$prog" "$@" 2>/dev/null)" || true
    assert_eq "$id" "$desc" "$fk_out" "$awk_out"
}

# sorted comparison (for hash-order-dependent output)
compat_sorted() {
    local id="$1" desc="$2" prog="$3"
    shift 3
    local awk_out fk_out
    awk_out="$($AWK "$prog" "$@" 2>/dev/null | sort)" || true
    fk_out="$($FK  "$prog" "$@" 2>/dev/null | sort)" || true
    assert_eq "$id" "$desc" "$fk_out" "$awk_out"
}

compat_sorted_flags() {
    local id="$1" desc="$2"
    shift 2
    local args=()
    while [[ "$1" != "--" ]]; do args+=("$1"); shift; done
    shift
    local prog="$1"; shift
    local awk_out fk_out
    awk_out="$($AWK "${args[@]}" "$prog" "$@" 2>/dev/null | sort)" || true
    fk_out="$($FK  "${args[@]}" "$prog" "$@" 2>/dev/null | sort)" || true
    assert_eq "$id" "$desc" "$fk_out" "$awk_out"
}

# ── generate test data ──────────────────────────────────────────────

W="$TMPDIR_SUITE"
gen_words    > "$W/words.txt"
gen_numbers  > "$W/numbers.txt"
gen_scores   > "$W/scores.txt"
gen_log      > "$W/log.txt"
gen_csv      > "$W/data.csv"
gen_lookup   > "$W/lookup.txt"
gen_config   > "$W/config.txt"
gen_text     > "$W/text.txt"
gen_file1    > "$W/f1.txt"
gen_file2    > "$W/f2.txt"
gen_mixed    > "$W/mixed.txt"
gen_prices   > "$W/prices.txt"
gen_orders   > "$W/orders.txt"
gen_logfile  > "$W/logfile.txt"
gen_subst    > "$W/subst.txt"
gen_sparse   > "$W/sparse.txt"

# multi-field numeric for stats
printf "10\n20\n30\n40\n50\n" > "$W/nums5.txt"
# wider data
printf "a 1 x\nb 2 y\nc 3 z\nd 4 x\ne 5 y\n" > "$W/wide.txt"
# data with > 4 fields
printf "a b c d e f\ng h\ni j k l m\n" > "$W/fields.txt"
# data for field 5 test
printf "1 2 3 4 abc123 6\n1 2 3 4 xyz 6\n1 2 3 4 abc123 7\n" > "$W/f5.txt"
# data for field 7 regex test
printf "1 2 3 4 5 6 alpha\n1 2 3 4 5 6 Beta\n1 2 3 4 5 6 foo\n" > "$W/f7.txt"
# lines of various lengths
printf "short\n%s\n%s\nhi\n" "$(printf 'x%.0s' {1..70})" "$(printf 'y%.0s' {1..50})" > "$W/lengths.txt"
# Beth test data
printf "Beth is here\nNo one\nBeth again\nStill no\n" > "$W/beth.txt"
# large first field test
printf "5 alpha\n12 beta\n3 gamma\n99 delta\n" > "$W/maxfield.txt"

# ════════════════════════════════════════════════════════════════════
section "1. Unique values"
# ════════════════════════════════════════════════════════════════════

compat "1" "unique lines, preserve order" \
    '!seen[$0]++' "$W/words.txt"

compat_sorted "3" "frequency count" \
    '{ a[$1]++ } END { for(k in a) print a[k], k }' "$W/words.txt"

compat "5" "count distinct" \
    '{ a[$1]=1 } END { n=0; for(k in a) n++; print n }' "$W/words.txt"

compat "6" "unique multi-column" \
    '!seen[$1,$2]++' "$W/scores.txt"

compat_sorted "7" "top N frequency (piped)" \
    '{ a[$1]++ } END { for(k in a) print a[k],k }' "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "2. Sorting"
# ════════════════════════════════════════════════════════════════════

# asort is gawk-only; use a program both awk and fk support
compat "11" "reverse lines (simple)" \
    '{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }' "$W/numbers.txt"

# ════════════════════════════════════════════════════════════════════
section "3. Frequency counting"
# ════════════════════════════════════════════════════════════════════

compat_sorted "14" "basic frequency count" \
    '{ a[$1]++ } END { for(k in a) print a[k], k }' "$W/words.txt"

compat_sorted "17" "percentage breakdown" \
    '{ a[$1]++; t++ } END { for(k in a) printf "%s: %.1f%%\n", k, a[k]/t*100 }' "$W/words.txt"

compat_sorted "19" "word frequency all fields" \
    '{ for(i=1;i<=NF;i++) a[$i]++ } END { for(k in a) print a[k], k }' "$W/text.txt"

# ════════════════════════════════════════════════════════════════════
section "4. Set operations"
# ════════════════════════════════════════════════════════════════════

compat "20" "lines in f1 not f2" \
    'NR==FNR{a[$0]=1;next} !($0 in a)' "$W/f2.txt" "$W/f1.txt"

compat "22" "union (unique lines)" \
    '!seen[$0]++' "$W/f1.txt" "$W/f2.txt"

# ════════════════════════════════════════════════════════════════════
section "5. Array manipulation"
# ════════════════════════════════════════════════════════════════════

compat "28" "reverse array" \
    '{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }' "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "6. Statistics"
# ════════════════════════════════════════════════════════════════════

compat "37" "mean" \
    '{ s+=$1 } END { print s/NR }' "$W/numbers.txt"

compat_sorted "42" "group-by mean" \
    '{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }' "$W/scores.txt"

# ════════════════════════════════════════════════════════════════════
section "7. File and I/O"
# ════════════════════════════════════════════════════════════════════

compat "45" "lookup join" \
    'NR==FNR{a[$1]=$2;next} ($1 in a){print $0, a[$1]}' "$W/lookup.txt" "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "8. String formatting"
# ════════════════════════════════════════════════════════════════════

compat "51" "right-align printf" \
    '{ printf "%10s %s\n", $1, $2 }' "$W/scores.txt"

compat "52" "left-align printf" \
    '{ printf "%-20s %s\n", $1, $2 }' "$W/scores.txt"

compat "58" "truncate with ellipsis" \
    '{ s=$0; if(length(s)>30) s=substr(s,1,27)"..."; print s }' "$W/text.txt"

# ════════════════════════════════════════════════════════════════════
section "9. Data transformation"
# ════════════════════════════════════════════════════════════════════

compat_sorted "60" "pivot group-collect" \
    '{ a[$1]=a[$1] (a[$1]?",":"") $2 } END { for(k in a) print k, a[k] }' "$W/scores.txt"

compat "65" "replace via lookup map" \
    'NR==FNR{m[$1]=$2;next} {for(i=1;i<=NF;i++) if($i in m) $i=m[$i]; print}' "$W/prices.txt" "$W/words.txt"

compat "67" "build CSV from arrays" \
    '{ a[NR]=$1; b[NR]=$2 } END { for(i=1;i<=NR;i++) print a[i]","b[i] }' "$W/scores.txt"

# ════════════════════════════════════════════════════════════════════
section "10. Multi-file processing"
# ════════════════════════════════════════════════════════════════════

compat "68" "enrich from lookup" \
    'NR==FNR { lu[$1]=$2; next } { print $0, ($1 in lu ? lu[$1] : "N/A") }' "$W/prices.txt" "$W/orders.txt"

compat "69" "anti-join" \
    'NR==FNR{a[$0]=1;next} !($0 in a)' "$W/f2.txt" "$W/f1.txt"

compat "70" "semi-join" \
    'NR==FNR{a[$1]=1;next} $1 in a' "$W/f2.txt" "$W/f1.txt"

compat_sorted "72" "compare configs" \
    'NR==FNR{a[$1]=$2;next} $1 in a && a[$1]!=$2{print $1,"old="a[$1],"new="$2}' "$W/config.txt" "$W/config.txt"

# ════════════════════════════════════════════════════════════════════
section "11. Random & sampling"
# ════════════════════════════════════════════════════════════════════

# Can't compare random output directly; just verify same line count
fk_shuf="$($FK '{ a[NR]=$0 } END { shuf(a); print a }' "$W/numbers.txt" | wc -l | tr -d ' ')"
assert_eq "75" "shuffle preserves line count" "$fk_shuf" "10"

# ════════════════════════════════════════════════════════════════════
section "12. Complex patterns"
# ════════════════════════════════════════════════════════════════════

compat_sorted "85" "cross-tabulation" \
    '{ a[$1 FS $2]++ } END { for(k in a) print k, a[k] }' "$W/log.txt"

compat "86" "running distinct count" \
    '{ seen[$1]=1; n=0; for(k in seen) n++; print NR, n }' "$W/words.txt"

compat "88" "sliding window avg" \
    '{ a[NR]=$1 } NR>=3 { printf "%.4f\n", (a[NR]+a[NR-1]+a[NR-2])/3 }' "$W/numbers.txt"

compat_sorted "90" "mode" \
    '{ a[$1]++ } END { max=0; for(k in a) if(a[k]>max){max=a[k];mode=k} print mode }' "$W/words.txt"

compat_sorted "93" "reverse lookup table" \
    '{ a[$2]=$1 } END { for(k in a) print k, a[k] }' "$W/lookup.txt"

# ════════════════════════════════════════════════════════════════════
section "P. Pement one-liners — file spacing"
# ════════════════════════════════════════════════════════════════════

compat "P1" "double space" \
    '{print; print ""}' "$W/words.txt"

compat "P2" "double space (ORS)" \
    'BEGIN{ORS="\n\n"};1' "$W/words.txt"

compat "P3" "double space, skip existing blanks" \
    'NF{print $0 "\n"}' "$W/sparse.txt"

compat "P4" "triple space" \
    '{print; print "\n"}' "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "P. Pement — numbering & calculations"
# ════════════════════════════════════════════════════════════════════

compat "P5" "number lines (FNR)" \
    '{print FNR "\t" $0}' "$W/words.txt"

compat "P6" "number lines (NR)" \
    '{print NR "\t" $0}' "$W/words.txt" "$W/numbers.txt"

compat "P7" "number right-aligned" \
    '{printf "%5d : %s\n", NR, $0}' "$W/words.txt"

compat "P8" "number non-blank lines" \
    'NF{$0=++a " :" $0};1' "$W/sparse.txt"

compat "P9" "count lines (wc -l)" \
    'END{print NR}' "$W/words.txt"

compat "P10" "sum fields per line" \
    '{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}' "$W/numbers.txt"

compat "P11" "sum all fields" \
    '{for (i=1; i<=NF; i++) s=s+$i}; END{print s+0}' "$W/numbers.txt"

compat "P12" "absolute value of fields" \
    '{for (i=1; i<=NF; i++) if ($i < 0) $i = -$i; print }' "$W/numbers.txt"

compat "P13" "total field count" \
    '{ total = total + NF }; END {print total}' "$W/text.txt"

compat "P14" "count lines matching Beth" \
    '/Beth/{n++}; END {print n+0}' "$W/beth.txt"

compat "P15" "largest first field" \
    '$1 > max {max=$1; maxline=$0}; END{ print max, maxline}' "$W/maxfield.txt"

compat "P16" "field count per line" \
    '{ print NF ":" $0 }' "$W/text.txt"

compat "P17" "last field each line" \
    '{ print $NF }' "$W/scores.txt"

compat "P18" "last field of last line" \
    '{ field = $NF }; END{ print field }' "$W/scores.txt"

compat "P19" "lines with > 4 fields" \
    'NF > 4' "$W/fields.txt"

compat "P20" "last field > 4" \
    '$NF > 4' "$W/maxfield.txt"

# ════════════════════════════════════════════════════════════════════
section "P. Pement — text conversion & substitution"
# ════════════════════════════════════════════════════════════════════

compat "P22" "strip CR" \
    '{sub(/\r$/,"")};1' "$W/words.txt"

compat "P23" "ltrim" \
    '{sub(/^[ \t]+/, "")};1' "$W/text.txt"

compat "P24" "rtrim" \
    '{sub(/[ \t]+$/, "")};1' "$W/text.txt"

compat "P25" "trim both" \
    '{gsub(/^[ \t]+|[ \t]+$/,"")};1' "$W/text.txt"

compat "P26" "right-align 79 cols" \
    '{printf "%79s\n", $0}' "$W/words.txt"

compat "P27" "sub first foo" \
    '{sub(/foo/,"bar")} {print}' "$W/subst.txt"

compat "P28" "gsub all foo" \
    '{gsub(/foo/,"bar")} {print}' "$W/subst.txt"

compat "P30" "sub on lines with baz" \
    '/baz/{gsub(/foo/, "bar")} {print}' "$W/subst.txt"

compat "P31" "sub on lines without baz" \
    '!/baz/{gsub(/foo/, "bar")} {print}' "$W/subst.txt"

compat "P32" "multi-pattern gsub" \
    '{gsub(/scarlet|ruby|puce/, "red")}; 1' "$W/text.txt"

compat "P33" "reverse lines (tac)" \
    '{a[i++]=$0} END {for (j=i-1; j>=0;) print a[j--] }' "$W/words.txt"

compat "P35" "swap first 2 fields" \
    '{print $2, $1}' "$W/scores.txt"

compat "P36" "delete second field" \
    '{ $2 = ""; print }' "$W/scores.txt"

compat "P37" "reverse field order" \
    '{for (i=NF; i>0; i--) printf "%s ",$i; print ""}' "$W/wide.txt"

compat "P38" "concat every 5 lines" \
    'ORS=NR%5?",":"\n"' "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "P. Pement — selective printing"
# ════════════════════════════════════════════════════════════════════

compat "P39" "first 10 lines (head)" \
    'NR < 11' "$W/words.txt"

compat "P40" "first line" \
    'NR>1{exit};1' "$W/words.txt"

compat "P41" "last 2 lines" \
    '{y=x "\n" $0; x=$0};END{print y}' "$W/words.txt"

compat "P42" "last line" \
    'END{print}' "$W/words.txt"

compat "P43" "grep regex" \
    '/apple/' "$W/words.txt"

compat "P44" "grep -v" \
    '!/apple/' "$W/words.txt"

compat "P45" "field equals string" \
    '$5 == "abc123"' "$W/f5.txt"

compat "P46" "field matches regex" \
    '$7 ~ /^[a-f]/' "$W/f7.txt"

compat "P47" "line before match" \
    '/cherry/{print x};{x=$0}' "$W/words.txt"

compat "P49" "AND grep" \
    '$0 ~ /quick/ && $0 ~ /fox/' "$W/text.txt"

compat "P50" "ordered grep" \
    '/quick.*fox/' "$W/text.txt"

compat "P51" "lines longer than 64" \
    'length($0) > 64' "$W/lengths.txt"

compat "P53" "lines 2 to 4" \
    'NR==2,NR==4' "$W/words.txt"

compat "P54" "print specific line" \
    'NR==3 {print;exit}' "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "P. Pement — selective deletion"
# ════════════════════════════════════════════════════════════════════

compat "P56" "delete blank lines" \
    'NF' "$W/sparse.txt"

compat "P57" "remove consecutive dupes (uniq)" \
    'a != $0 {print} {a=$0}' "$W/words.txt"

compat "P58" "remove all dupes" \
    '!a[$0]++' "$W/words.txt"

# ════════════════════════════════════════════════════════════════════
section "C. Two-file idioms"
# ════════════════════════════════════════════════════════════════════

compat "C1" "lookup join" \
    'NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}' "$W/prices.txt" "$W/orders.txt"

compat "C2" "anti-join" \
    'NR==FNR{skip[$1]=1; next} !($1 in skip)' "$W/f2.txt" "$W/f1.txt"

compat "C3" "semi-join" \
    'NR==FNR{keep[$1]=1; next} $1 in keep' "$W/f2.txt" "$W/f1.txt"

compat "C4" "diff — lines in f1 not f2" \
    'NR==FNR{a[$0]=1; next} !($0 in a)' "$W/f2.txt" "$W/f1.txt"

compat "C5" "update from second file" \
    'NR==FNR{a[$1]=$2; next} {if($1 in a) $2=a[$1]; print}' "$W/lookup.txt" "$W/scores.txt"

# ════════════════════════════════════════════════════════════════════
print_summary "compat"
