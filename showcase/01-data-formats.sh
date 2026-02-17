#!/usr/bin/env bash
# 01 — CSV, TSV, JSON, headers, jpath, nested JSON
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. CSV with named columns — no csvkit, no counting fields"
# awk needs: csvkit to parse, then count which column is which.
# fk: -i csv -H gives you $column_name directly.

show $FK -i csv -H '{
    rev[$region] += $revenue
}
END {
    for (r in rev) printf "  %-6s $%.0f\n", r, rev[r]
}' "$TMPDIR/sales.csv"

section "2. Quoted column names — hyphens, dots, anything"
# awk: impossible. fk: $"col-name" for any header.

show $FK -i csv -H '{
    cpu = $"cpu-usage" + 0; mem = $"mem-usage" + 0
    status = "OK"
    if (cpu > 90 || mem > 90) status = "CRITICAL"
    else if (cpu > 70 || mem > 70) status = "WARNING"
    printf "  %-12s cpu=%5.1f%%  mem=%5.1f%%  [%s]\n", $"host-name", cpu, mem, status
}' "$TMPDIR/servers.csv"

section "3. JSON Lines with jpath() — no jq needed"
# awk: zero JSON support. fk: jpath() navigates objects and arrays.

show $FK '{
    method = jpath($0, ".method"); status = jpath($0, ".status") + 0
    ms = jpath($0, ".ms") + 0; path = jpath($0, ".path")
    tag = ""
    if (status >= 500) tag = " ** ERROR"
    if (ms > 100) tag = tag " SLOW"
    printf "  %-6s %-20s %3d %4dms%s\n", method, path, status, ms, tag
}' "$TMPDIR/api.jsonl"

section "4. Nested JSON iteration"
# awk: completely impossible. fk: jpath with array extraction.

show_pipe "echo '{\"team\":\"eng\",\"members\":[{\"name\":\"Alice\",\"role\":\"lead\"},{\"name\":\"Bob\",\"role\":\"dev\"},{\"name\":\"Carol\",\"role\":\"dev\"}]}' | $FK '{
    team = jpath(\$0, \".team\")
    n = jpath(\$0, \".members\", m)
    for (i=1; i<=n; i++) printf \"  %s: %s (%s)\n\", team, jpath(m[i], \".name\"), jpath(m[i], \".role\")
}'"

printf "\n${C_BOLD}Done.${C_RESET} fk handles CSV, TSV, JSON natively — awk needs external tools.\n"
