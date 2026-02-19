# Performance Baseline (Strict)

Baseline captured with `tests/suite/perf_strict.sh` (warmup=1, reps=9).
Report file: `bench_data/perf_strict_1000000_20260219-113328.txt`.

## fk vs awk (1,000,000 lines)

| Benchmark | awk median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| print $2 | 0.624s | 0.137s | 4.55x |
| sum column | 0.579s | 0.213s | 2.72x |
| pattern match | 0.730s | 0.197s | 3.70x |
| field arithmetic | 0.887s | 0.346s | 2.57x |
| associative array | 0.646s | 0.265s | 2.44x |
| frequency count | 0.269s | 0.238s | 1.13x |
| gsub | 0.992s | 0.426s | 2.33x |
| NR==FNR join | 1.546s | 0.495s | 3.12x |

## fk vs tools (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| wc -l | 0.045s | 0.094s | 0.48x |
| grep pattern | 0.193s | 0.165s | 1.17x |
| cut -d' ' -f2 | 0.278s | 0.143s | 1.94x |
| head -100 | 0.022s | 0.043s | 0.52x |
| uniq (sorted input) | 0.167s | 0.236s | 0.71x |

## fk-only features (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| mean() builtin | 0.234s | 0.521s | 0.45x |
| median() builtin | 1.052s | 0.536s | 1.96x |
| reverse() builtin | 0.151s | 0.193s | 0.78x |

## Top Regression Targets

- B14 `mean() builtin` (0.45x)
- B9 `wc -l` (0.48x)
- B12 `head -100` (0.52x)
