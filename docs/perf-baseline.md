# Performance Baseline (Strict)

Baseline captured with `tests/suite/perf_strict.sh` (warmup=1, reps=9).
Report file: `bench_data/perf_strict_1000000_20260219-111210.txt`.

## fk vs awk (1,000,000 lines)

| Benchmark | awk median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| print $2 | 0.614s | 0.135s | 4.53x |
| sum column | 0.554s | 0.211s | 2.63x |
| /active/ count | 0.711s | 0.198s | 3.60x |
| field arithmetic | 0.876s | 0.338s | 2.59x |
| associative array | 0.632s | 0.257s | 2.46x |
| frequency count | 0.263s | 0.231s | 1.14x |
| gsub | 0.984s | 0.413s | 2.38x |
| NR==FNR join | 1.498s | 0.493s | 3.04x |

## fk vs tools (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| wc -l | 0.045s | 0.119s | 0.37x |
| grep pattern | 0.180s | 0.159s | 1.13x |
| cut -d' ' -f2 | 0.281s | 0.134s | 2.09x |
| head -100 | 0.021s | 0.039s | 0.54x |
| uniq (sorted input) | 0.167s | 0.238s | 0.70x |

## fk-only features (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| mean() builtin | 0.230s | 0.519s | 0.44x |
| median() builtin | 1.045s | 0.517s | 2.02x |
| reverse() builtin | 0.145s | 0.186s | 0.78x |

## Top Regression Targets

- B9 `wc -l` (0.37x)
- B14 `mean()` builtin (0.44x)
- B12 `head -100` (0.54x)

