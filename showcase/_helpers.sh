# Shared helpers for showcase scripts.
# Source this file, don't run it directly.

FK="${FK:-$(dirname "$0")/../target/release/fk}"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

HAS_UPLOT=false
command -v uplot >/dev/null 2>&1 && HAS_UPLOT=true

# ── Color codes ────────────────────────────────────────────────────
C_RESET=$'\033[0m'
C_BOLD=$'\033[1m'
C_DIM=$'\033[2m'
C_CYAN=$'\033[1;96m'
C_YEL=$'\033[93m'
C_FLAG=$'\033[33m'
C_FILE=$'\033[37m'
C_SEC=$'\033[1;36m'

section() { printf "\n${C_SEC}━━ %s ━━${C_RESET}\n\n" "$1"; }

# Print the full fk command (program + switches), then run it.
show() {
    local flags="" prog="" files="" found_prog=false
    for arg in "$@"; do
        if [[ "$arg" == "$FK" ]]; then
            continue
        elif [[ "$found_prog" == false && "$arg" == -* ]]; then
            flags+=" ${C_FLAG}${arg}${C_RESET}"
        elif [[ "$found_prog" == false && ("$arg" == *"{"* || "$arg" == *"/"* && -f "$arg") ]]; then
            if [[ -f "$arg" ]]; then
                files+=" ${C_FILE}$(basename "$arg")${C_RESET}"
            else
                prog="$arg"
                found_prog=true
            fi
        elif $found_prog && [[ -f "$arg" ]]; then
            files+=" ${C_FILE}$(basename "$arg")${C_RESET}"
        elif $found_prog; then
            files+=" ${C_FILE}${arg}${C_RESET}"
        else
            flags+=" ${C_FLAG}${arg}${C_RESET}"
        fi
    done

    printf "\n  ${C_DIM}\$${C_RESET} ${C_CYAN}${C_BOLD}fk${C_RESET}%s" "$flags"
    [[ -n "$files" ]] && printf " %b" "$files"

    if [[ -n "$prog" ]]; then
        printf " ${C_YEL}'${C_RESET}\n"
        while IFS= read -r line; do
            printf "    ${C_YEL}%s${C_RESET}\n" "$line"
        done <<< "$prog"
        printf "  ${C_YEL}'${C_RESET}\n"
    else
        printf "\n"
    fi
    echo ""
    "$@"
}

# Print a readable pipeline description, then run it via eval.
show_pipe() {
    local desc="$1"
    local display="${desc//$FK/fk}"
    display="${display//$TMPDIR\//}"
    printf "\n  ${C_DIM}\$${C_RESET} ${C_YEL}%s${C_RESET}\n\n" "$display"
    eval "$desc"
}

# ── Shared test data ───────────────────────────────────────────────
setup_data() {
    cat > "$TMPDIR/sales.csv" <<'CSV'
region,product,revenue,units,quarter
EMEA,Widget,14500,230,Q1
APAC,Gadget,22300,410,Q2
NA,Widget,18700,350,Q1
EMEA,Gadget,9100,180,Q3
NA,Gizmo,31200,520,Q2
APAC,Widget,11800,200,Q4
EMEA,Gizmo,27400,460,Q3
NA,Gadget,16900,290,Q1
APAC,Gizmo,8600,150,Q4
CSV

    cat > "$TMPDIR/access.log" <<'LOG'
192.168.1.10 - - [16/Feb/2025:10:15:30 +0000] "GET /index.html HTTP/1.1" 200 1234
10.0.0.5 - admin [16/Feb/2025:10:15:31 +0000] "POST /api/login HTTP/1.1" 302 0
172.16.0.1 - - [16/Feb/2025:10:15:32 +0000] "GET /static/style.css HTTP/1.1" 200 8901
192.168.1.10 - - [16/Feb/2025:10:15:33 +0000] "GET /api/data HTTP/1.1" 500 45
10.0.0.5 - admin [16/Feb/2025:10:15:34 +0000] "DELETE /api/users/3 HTTP/1.1" 204 0
192.168.1.10 - - [16/Feb/2025:10:15:35 +0000] "GET /api/data HTTP/1.1" 200 2048
172.16.0.1 - - [16/Feb/2025:10:15:36 +0000] "GET /api/users HTTP/1.1" 200 5120
10.0.0.5 - admin [16/Feb/2025:10:15:37 +0000] "PUT /api/users/3 HTTP/1.1" 200 128
LOG

    cat > "$TMPDIR/events.csv" <<'CSV'
event,date,attendees
Kickoff,2025-01-15 09:00:00,45
Sprint Review,2025-02-01 14:00:00,30
Release Party,2025-02-16 18:00:00,120
Retrospective,2025-03-01 10:30:00,25
Offsite,2025-03-15 08:00:00,80
Hackathon,2025-04-01 10:00:00,60
CSV

    cat > "$TMPDIR/servers.csv" <<'CSV'
host-name,cpu-usage,mem-usage,disk.free,net.rx-bytes
web-01,72.5,85.3,120,984320
web-02,45.1,60.2,250,1230400
db-01,91.8,95.1,50,540200
cache-01,12.3,40.7,180,320100
worker-01,88.2,78.9,90,2100000
worker-02,34.5,52.1,300,1800000
CSV

    cat > "$TMPDIR/api.jsonl" <<'JSONL'
{"ts":"2025-02-16T10:00:01","method":"GET","path":"/api/users","status":200,"ms":12}
{"ts":"2025-02-16T10:00:02","method":"POST","path":"/api/users","status":201,"ms":45}
{"ts":"2025-02-16T10:00:03","method":"GET","path":"/api/users/42","status":200,"ms":8}
{"ts":"2025-02-16T10:00:04","method":"GET","path":"/api/products","status":500,"ms":1230}
{"ts":"2025-02-16T10:00:05","method":"DELETE","path":"/api/users/7","status":204,"ms":15}
{"ts":"2025-02-16T10:00:06","method":"GET","path":"/api/products","status":200,"ms":34}
{"ts":"2025-02-16T10:00:07","method":"POST","path":"/api/orders","status":201,"ms":89}
{"ts":"2025-02-16T10:00:08","method":"GET","path":"/api/users","status":200,"ms":11}
JSONL

    cat > "$TMPDIR/latencies.txt" <<'DATA'
12
45
8
1230
15
34
89
11
23
67
5
450
102
19
38
DATA

    cat > "$TMPDIR/orders.csv" <<'CSV'
order_id,customer,amount,currency,created_at
1001,Alice Smith,149.99,USD,2025-01-10 08:30:00
1002,Bob Jones,2340.00,EUR,2025-01-15 14:20:00
1003,Carol Wu,89.50,USD,2025-02-01 09:00:00
1004,Alice Smith,320.00,USD,2025-02-05 16:45:00
1005,Dan Lee,1100.00,GBP,2025-02-10 11:30:00
1006,Bob Jones,450.00,EUR,2025-02-14 13:00:00
1007,Eve Park,75.00,USD,2025-02-15 10:15:00
1008,Alice Smith,210.00,USD,2025-02-16 08:00:00
CSV

    printf "banana\napple\ncherry\ndate\nelderberry\nfig\ngrape\napricot\n" > "$TMPDIR/fruits.txt"
}
