#!/usr/bin/env bash
set -euo pipefail

FK="${1:?usage: bench-compare.sh <fk-binary> <data-file> <line-count>}"
DATA="${2:?}"
LINES="${3:?}"

# Time a command, suppress output, print formatted result
run() {
    local label="$1" task="$2"
    shift 2
    local start end elapsed
    start=$(python3 -c "import time; print(time.monotonic())")
    "$@" > /dev/null 2>&1 || true
    end=$(python3 -c "import time; print(time.monotonic())")
    elapsed=$(python3 -c "print(f'{$end - $start:.3f}s')")
    printf "  %-10s %-40s %s\n" "$label" "$task" "$elapsed"
}

echo ""
echo "═══ fk vs awk comparison ($LINES lines) ═══"
echo ""

# Collect tools
tools=("$FK" "awk")
names=("fk" "awk")
command -v gawk >/dev/null 2>&1 && { tools+=("gawk"); names+=("gawk"); }
command -v mawk >/dev/null 2>&1 && { tools+=("mawk"); names+=("mawk"); }

# Tasks
tasks=(
    "print \$2|{ print \$2 }"
    "sum column|-F, { s += \$3 } END { print s }"
    "/active/ count|-F, /active/ { c++ } END { print c }"
    "field arithmetic|-F, { x = \$1 + \$3 }"
    "associative array|-F, { a[\$2]++ } END { for (k in a) n++; print n }"
)

for task_spec in "${tasks[@]}"; do
    IFS='|' read -r task_label task_prog <<< "$task_spec"
    echo "── $task_label ──"

    for i in "${!tools[@]}"; do
        tool="${tools[$i]}"
        name="${names[$i]}"

        # Build argument list: split the leading flags from the program
        if [[ "$task_prog" == -F* ]]; then
            flag="${task_prog%% *}"
            prog="${task_prog#* }"
            run "$name" "$task_label" "$tool" "$flag" "$prog" "$DATA"
        else
            run "$name" "$task_label" "$tool" "$task_prog" "$DATA"
        fi
    done
    echo ""
done
