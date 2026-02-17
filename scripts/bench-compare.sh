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
    "computed regex|-F, BEGIN{p=\"user_4[0-9]{2}\"} \$2~p{c++} END{print c}"
    "tight loop (3x)|-F, { for(i=1;i<=3;i++) s+=\$3 } END { print s }"
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

# ── Parquet benchmark (fk only) ──────────────────────────────────

PARQUET_DATA="${DATA%.csv}.parquet"
FK_PARQUET="${FK}-parquet"

# Build parquet-enabled binary if not already present
if [ ! -f "$FK_PARQUET" ] || [ "$FK_PARQUET" -ot "src/main.rs" ]; then
    echo "Building fk with parquet support..."
    cargo build --release 2>/dev/null
    cp "$FK" "$FK_PARQUET" 2>/dev/null || true
fi

# Generate parquet test file if pyarrow is available
if command -v python3 >/dev/null 2>&1 && python3 -c "import pyarrow" 2>/dev/null; then
    if [ ! -f "$PARQUET_DATA" ] || [ "$PARQUET_DATA" -ot "$DATA" ]; then
        echo "Generating Parquet file from CSV..."
        python3 -c "
import pyarrow as pa
import pyarrow.csv as pc
import pyarrow.parquet as pq
table = pc.read_csv('$DATA', read_options=pc.ReadOptions(column_names=['id','user','value','status']))
pq.write_table(table, '$PARQUET_DATA')
print(f'  → {table.num_rows} rows written to $PARQUET_DATA')
"
    fi

    echo ""
    echo "═══ fk parquet vs fk csv ($LINES lines) ═══"
    echo ""

    parquet_tasks=(
        "sum column (csv)|-F, { s += \$3 } END { print s }|csv"
        "sum column (parquet)|-i parquet { s += \$value } END { print s }|parquet"
        "filter+count (csv)|-F, /active/ { c++ } END { print c }|csv"
        "filter+count (parquet)|-i parquet /active/ { c++ } END { print c }|parquet"
        "group-by (csv)|-F, { a[\$2]++ } END { for(k in a) n++; print n }|csv"
        "group-by (parquet)|-i parquet { a[\$user]++ } END { for(k in a) n++; print n }|parquet"
    )

    for task_spec in "${parquet_tasks[@]}"; do
        IFS='|' read -r task_label task_prog task_type <<< "$task_spec"

        if [[ "$task_type" == "parquet" ]]; then
            file="$PARQUET_DATA"
            # Parse the -i parquet prefix
            prog="${task_prog#-i parquet }"
            run "fk" "$task_label" "$FK_PARQUET" -i parquet "$prog" "$file"
        else
            file="$DATA"
            if [[ "$task_prog" == -F* ]]; then
                flag="${task_prog%% *}"
                prog="${task_prog#* }"
                run "fk" "$task_label" "$FK" "$flag" "$prog" "$file"
            else
                run "fk" "$task_label" "$FK" "$task_prog" "$file"
            fi
        fi
    done
    echo ""
else
    echo ""
    echo "(Skipping parquet benchmark — pyarrow not installed)"
    echo ""
fi
