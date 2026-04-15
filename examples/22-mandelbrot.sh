#!/usr/bin/env bash
# 22-mandelbrot.sh — Mandelbrot set: pure computation showcase
#
# Run: ./examples/22-mandelbrot.sh
set -euo pipefail
source "$(dirname "$0")/_helpers.sh"

echo "═══ Mandelbrot set — pure computation ═══"
echo ""

# ── ASCII Mandelbrot ────────────────────────────────────────────────
section "ASCII Mandelbrot (78×36, 80 iterations)"

show $FK '
BEGIN {
    W = 78; H = 36; max = 80
    chars = " .:-=+*#%@"
    nc = length(chars)
    for (row = 0; row < H; row++) {
        ci = (row / H) * 3.0 - 1.5
        line = ""
        for (col = 0; col < W; col++) {
            cr = (col / W) * 3.5 - 2.5
            zr = 0; zi = 0; iter = 0
            while (iter < max && zr**2 + zi**2 < 4) {
                tmp = zr**2 - zi**2 + cr
                zi = 2 * zr * zi + ci
                zr = tmp
                iter++
            }
            idx = int(iter / max * (nc - 1)) + 1
            line = line substr(chars, idx, 1)
        }
        print line
    }
}
'

echo ""

# ── 256-color half-block Mandelbrot ─────────────────────────────────
section "256-color half-block Mandelbrot (80\u00d764, 80 iterations)"

E=$(printf '\033')
show $FK -v "E=$E" '
BEGIN {
    W = 80; H = 64; max = 80
    half = "\u2580"
    for (row = 0; row < H; row += 2) {
        ci_top = (row / H) * 3.0 - 1.5
        ci_bot = ((row + 1) / H) * 3.0 - 1.5
        for (col = 0; col < W; col++) {
            cr = (col / W) * 3.5 - 2.5
            # top pixel
            zr = 0; zi = 0; it = 0
            while (it < max && zr**2 + zi**2 < 4) {
                t = zr**2 - zi**2 + cr
                zi = 2 * zr * zi + ci_top
                zr = t; it++
            }
            top = it
            # bottom pixel
            zr = 0; zi = 0; it = 0
            while (it < max && zr**2 + zi**2 < 4) {
                t = zr**2 - zi**2 + cr
                zi = 2 * zr * zi + ci_bot
                zr = t; it++
            }
            bot = it
            if (top == max) tc = 0; else tc = 16 + int(top / max * 215)
            if (bot == max) bc = 0; else bc = 16 + int(bot / max * 215)
            printf "%s[38;5;%d;48;5;%dm%s", E, tc, bc, half
        }
        printf "%s[0m\n", E
    }
}
'

echo ""

# ── Timed benchmark ────────────────────────────────────────────────
section "Benchmark: 160×80 @ 256 iterations"

ELAPSED=$( { time $FK '
BEGIN {
    W = 160; H = 80; max = 256
    for (row = 0; row < H; row++) {
        ci = (row / H) * 3.0 - 1.5
        for (col = 0; col < W; col++) {
            cr = (col / W) * 3.5 - 2.5
            zr = 0; zi = 0; iter = 0
            while (iter < max && zr**2 + zi**2 < 4) {
                tmp = zr**2 - zi**2 + cr
                zi = 2 * zr * zi + ci
                zr = tmp
                iter++
            }
        }
    }
}
' < /dev/null > /dev/null; } 2>&1 | grep real | sed 's/.*0m//;s/s$//' )
AWK_ELAPSED=$( { time awk '
BEGIN {
    W = 160; H = 80; max = 256
    for (row = 0; row < H; row++) {
        ci = (row / H) * 3.0 - 1.5
        for (col = 0; col < W; col++) {
            cr = (col / W) * 3.5 - 2.5
            zr = 0; zi = 0; iter = 0
            while (iter < max && zr * zr + zi * zi < 4) {
                tmp = zr * zr - zi * zi + cr
                zi = 2 * zr * zi + ci
                zr = tmp
                iter++
            }
        }
    }
}
' < /dev/null > /dev/null; } 2>&1 | grep real | sed 's/.*0m//;s/s$//' )
GAWK_PROG='
BEGIN {
    W = 160; H = 80; max = 256
    for (row = 0; row < H; row++) {
        ci = (row / H) * 3.0 - 1.5
        for (col = 0; col < W; col++) {
            cr = (col / W) * 3.5 - 2.5
            zr = 0; zi = 0; iter = 0
            while (iter < max && zr * zr + zi * zi < 4) {
                tmp = zr * zr - zi * zi + cr
                zi = 2 * zr * zi + ci
                zr = tmp
                iter++
            }
        }
    }
}'
GAWK_LINE=""
if command -v gawk >/dev/null 2>&1; then
    GAWK_ELAPSED=$( { time gawk "$GAWK_PROG" < /dev/null > /dev/null; } 2>&1 | grep real | sed 's/.*0m//;s/s$//' )
    GAWK_LINE="  gawk: ${GAWK_ELAPSED}s"
fi
echo "  160×80 grid, 256 max iterations"
echo "  ($(( 160 * 80 )) pixels, up to $(( 160 * 80 * 256 )) complex multiplications)"
echo ""
echo "  fk:   ${ELAPSED}s"
echo "  awk:  ${AWK_ELAPSED}s"
if [[ -n "$GAWK_LINE" ]]; then echo "$GAWK_LINE"; fi

echo ""
echo "  Compute is on par. Startup delta is binary page-in:"
echo "  fk ships parquet/arrow/zstd statically linked."
echo ""
printf "${C_BOLD}Done.${C_RESET} Mandelbrot in fk — no external tools, no I/O, just math.\n"
