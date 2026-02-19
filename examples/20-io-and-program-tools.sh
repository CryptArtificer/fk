#!/usr/bin/env bash
# 20 — IO + program tools: sub/gsub, getline, RS regex, ARGV/ARGC, format/highlight
#
# Story: you need controlled reads, in-place edits, and ways to inspect programs.
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"
setup_data

section "1. sub / gsub — in-place edits"

echo "sub(): replace the first match only"
show_pipe "echo 'user=alice token=abc token=def' | $FK '{
    sub(\"token=[^ ]+\", \"token=***\")
    print \"  \", \$0
}'"

echo ""
echo "gsub(): replace all matches"
show_pipe "echo 'x x x' | $FK '{ gsub(\"x\", \"y\"); print \"  \", \$0 }'"

section "2. getline + close — controlled reads"

cat > "$TMPDIR/config.txt" <<'EOF'
host=api.local
port=8443
EOF

echo "Read a file twice by closing the handle:"
show $FK -v "file=$TMPDIR/config.txt" 'BEGIN {
    while ((getline line < file) > 0) print "  first:", line
    close(file)
    while ((getline line < file) > 0) print "  second:", line
}'

echo ""
echo "Read from a command pipe, then close it:"
show $FK 'BEGIN {
    cmd = "printf \"one\\ntwo\\n\""
    while ((cmd | getline line) > 0) print "  pipe:", line
    close(cmd)
}'

section "3. RS as regex — split into paragraphs"

show_pipe "printf 'alpha\\nline\\n\\nbeta\\n\\n\\ngamma\\n' | $FK 'BEGIN { RS=\"\\\\n\\\\n+\" } {
    gsub(\"\\\\n\", \" \")
    print \"  paragraph:\", \$0
}'"

section "4. ARGC / ARGV — see input arguments"

printf "a\n" > "$TMPDIR/a.txt"
printf "b\n" > "$TMPDIR/b.txt"

show $FK 'BEGIN {
    print \"  ARGC:\", ARGC
    for (i = 0; i < ARGC; i++) print \"  ARGV[\" i \"]:\", ARGV[i]
}' "$TMPDIR/a.txt" "$TMPDIR/b.txt"

section "5. --format / --highlight — inspect programs"

cat > "$TMPDIR/demo.fk" <<'EOF'
BEGIN{ if (NR==0) print "never" }
{ total += $2; if ($2 > 10) print $1 }
END{ print "total:", total }
EOF

echo "Pretty-print a program:"
show_pipe "$FK --format -f $TMPDIR/demo.fk"

echo ""
echo "Syntax-highlight a program:"
show_pipe "$FK --highlight -f $TMPDIR/demo.fk"

printf "\n${C_BOLD}Done.${C_RESET} sub/gsub, getline/close, RS regex, ARGV/ARGC, format/highlight.\n"
