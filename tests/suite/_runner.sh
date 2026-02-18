#!/usr/bin/env bash
# _runner.sh — shared test harness for fk test suites
#
# Source this file from each suite script.
# Provides: data generators, assertion helpers, summary reporting.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
FK="${FK:-$ROOT_DIR/target/release/fk}"
TMPDIR_SUITE="$(mktemp -d)"

# counters
_pass=0 _fail=0 _skip=0
_failures=()

# ── colors ──────────────────────────────────────────────────────────

if [[ -t 1 ]]; then
    GREEN=$'\033[32m' RED=$'\033[31m' YELLOW=$'\033[33m'
    BOLD=$'\033[1m' DIM=$'\033[2m' RESET=$'\033[0m'
else
    GREEN="" RED="" YELLOW="" BOLD="" DIM="" RESET=""
fi

# ── cleanup ─────────────────────────────────────────────────────────

cleanup() { rm -rf "$TMPDIR_SUITE"; }
trap cleanup EXIT

# ── assertions ──────────────────────────────────────────────────────

# assert_eq ID "description" "actual" "expected"
assert_eq() {
    local id="$1" desc="$2" actual="$3" expected="$4"
    if [[ "$actual" == "$expected" ]]; then
        printf "  ${GREEN}✓${RESET} %-6s %s\n" "$id" "$desc"
        ((_pass++)) || true
    else
        printf "  ${RED}✗${RESET} %-6s %s\n" "$id" "$desc"
        printf "    ${DIM}expected:${RESET} %s\n" "$(head -c 200 <<< "$expected")"
        printf "    ${DIM}actual:  ${RESET} %s\n" "$(head -c 200 <<< "$actual")"
        ((_fail++)) || true
        _failures+=("$id: $desc")
    fi
}

# assert_sorted_eq ID "description" "actual" "expected"
# Sorts both before comparing (for hash-order-dependent output)
assert_sorted_eq() {
    local id="$1" desc="$2"
    local actual="$(sort <<< "$3")"
    local expected="$(sort <<< "$4")"
    assert_eq "$id" "$desc" "$actual" "$expected"
}

# assert_match ID "description" "actual" "pattern"
assert_match() {
    local id="$1" desc="$2" actual="$3" pattern="$4"
    if [[ "$actual" =~ $pattern ]]; then
        printf "  ${GREEN}✓${RESET} %-6s %s\n" "$id" "$desc"
        ((_pass++)) || true
    else
        printf "  ${RED}✗${RESET} %-6s %s\n" "$id" "$desc"
        printf "    ${DIM}pattern:${RESET} %s\n" "$pattern"
        printf "    ${DIM}actual: ${RESET} %s\n" "$(head -c 200 <<< "$actual")"
        ((_fail++)) || true
        _failures+=("$id: $desc")
    fi
}

# assert_nonzero ID "description" "actual"
# Passes if actual is non-empty
assert_nonzero() {
    local id="$1" desc="$2" actual="$3"
    if [[ -n "$actual" ]]; then
        printf "  ${GREEN}✓${RESET} %-6s %s\n" "$id" "$desc"
        ((_pass++)) || true
    else
        printf "  ${RED}✗${RESET} %-6s %s\n" "$id" "$desc"
        printf "    ${DIM}(empty output)${RESET}\n"
        ((_fail++)) || true
        _failures+=("$id: $desc")
    fi
}

# skip_test ID "description" "reason"
skip_test() {
    local id="$1" desc="$2" reason="$3"
    printf "  ${YELLOW}○${RESET} %-6s %s ${DIM}(%s)${RESET}\n" "$id" "$desc" "$reason"
    ((_skip++)) || true
}

# ── section headers ─────────────────────────────────────────────────

section() {
    printf "\n${BOLD}━━ %s ━━${RESET}\n" "$1"
}

# ── data generators ─────────────────────────────────────────────────

# Simple word file: one word per line
gen_words() {
    cat <<'DATA'
apple
banana
cherry
apple
date
banana
apple
elderberry
fig
date
DATA
}

# Numeric data: one number per line
gen_numbers() {
    cat <<'DATA'
42
17
93
8
55
31
76
42
19
64
DATA
}

# Two-column data: name score
gen_scores() {
    cat <<'DATA'
alice 88
bob 72
carol 95
alice 91
bob 68
dave 84
carol 77
dave 92
alice 79
bob 85
DATA
}

# Three-column data for frequency/grouping
gen_log() {
    cat <<'DATA'
web GET 200
api POST 201
web GET 404
api GET 200
web POST 500
web GET 200
api POST 201
web GET 200
api GET 404
web POST 200
DATA
}

# CSV data with header
gen_csv() {
    cat <<'DATA'
name,dept,salary
alice,eng,95000
bob,sales,72000
carol,eng,105000
dave,sales,68000
eve,eng,88000
frank,hr,74000
grace,hr,71000
DATA
}

# Two-column lookup file
gen_lookup() {
    cat <<'DATA'
apple red
banana yellow
cherry red
date brown
elderberry purple
DATA
}

# Key=value config
gen_config() {
    cat <<'DATA'
host=localhost
port=8080
debug=true
timeout=30
DATA
}

# Multi-field text
gen_text() {
    cat <<'DATA'
the quick brown fox jumps over the lazy dog
now is the time for all good men
to be or not to be that is the question
the quick brown fox runs fast
all good things come to those who wait
DATA
}

# Two separate files for set operations
gen_file1() { printf "alpha\nbeta\ngamma\ndelta\n"; }
gen_file2() { printf "beta\ndelta\nepsilon\nzeta\n"; }

# Sparse numbers with empties
gen_sparse() {
    printf "5\n\n3\n\n8\n1\n\n7\n"
}

# Multi-field with mixed numeric
gen_mixed() {
    cat <<'DATA'
server1 cpu 45.2
server2 cpu 78.1
server1 mem 62.3
server2 mem 89.5
server1 cpu 52.1
server2 cpu 81.3
server1 mem 67.8
server2 mem 92.1
DATA
}

# Two-column for lookup join (prices)
gen_prices() {
    cat <<'DATA'
apple 1.50
banana 0.75
cherry 3.00
date 5.00
DATA
}

# Orders referencing prices
gen_orders() {
    cat <<'DATA'
apple 10
banana 25
cherry 5
fig 8
DATA
}

# Lines with pattern for grep-like tests
gen_logfile() {
    cat <<'DATA'
2024-01-01 INFO Starting up
2024-01-01 ERROR Connection refused
2024-01-02 INFO Processing request
2024-01-02 WARN Slow query detected
2024-01-03 ERROR Disk full
2024-01-03 INFO Recovering
2024-01-04 ERROR Timeout occurred
2024-01-04 INFO Shutdown complete
DATA
}

# Lines with foo/bar/baz for substitution tests
gen_subst() {
    cat <<'DATA'
the foo is foo here
baz has foo in it
nothing special
foo at the start
end with foo
baz and foo and foo
DATA
}

# ── summary ─────────────────────────────────────────────────────────

print_summary() {
    local suite_name="${1:-suite}"
    printf "\n${BOLD}━━ %s summary ━━${RESET}\n" "$suite_name"
    printf "  ${GREEN}✓ %d passed${RESET}" "$_pass"
    [[ $_fail -gt 0 ]] && printf "  ${RED}✗ %d failed${RESET}" "$_fail"
    [[ $_skip -gt 0 ]] && printf "  ${YELLOW}○ %d skipped${RESET}" "$_skip"
    printf "  (total: %d)\n" "$((_pass + _fail + _skip))"

    if [[ $_fail -gt 0 ]]; then
        printf "\n  ${RED}Failures:${RESET}\n"
        for f in "${_failures[@]}"; do
            printf "    ${RED}•${RESET} %s\n" "$f"
        done
        return 1
    fi
    return 0
}

# ── ensure fk is built ──────────────────────────────────────────────

ensure_fk() {
    if [[ ! -x "$FK" ]]; then
        printf "${BOLD}Building fk (release)...${RESET}\n"
        (cd "$ROOT_DIR" && cargo build --release 2>/dev/null)
    fi
}
