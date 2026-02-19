# Performance Baseline (Strict)

Baseline captured with `tests/suite/perf_strict.sh` (warmup=1, reps=9).
Report file: `bench_data/perf_strict_1000000_20260219-114547.txt`.

## fk vs awk (1,000,000 lines)

| Benchmark | awk median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| print $2 | 0.628s | 0.138s | 4.57x |
| sum column | 0.572s | 0.209s | 2.74x |
| pattern match | 0.725s | 0.201s | 3.62x |
| field arithmetic | 0.873s | 0.353s | 2.47x |
| associative array | 0.634s | 0.264s | 2.40x |
| frequency count | 0.265s | 0.230s | 1.15x |
| gsub | 0.973s | 0.422s | 2.31x |
| NR==FNR join | 1.522s | 0.497s | 3.06x |

## fk vs tools (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| wc -l | 0.046s | 0.092s | 0.50x |
| grep pattern | 0.183s | 0.157s | 1.16x |
| cut -d' ' -f2 | 0.275s | 0.140s | 1.96x |
| head -100 | 0.020s | 0.040s | 0.50x |
| uniq (sorted input) | 0.162s | 0.237s | 0.68x |

## fk-only features (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| mean() builtin | 0.229s | 0.377s | 0.61x |
| median() builtin | 1.039s | 0.516s | 2.01x |
| reverse() builtin | 0.144s | 0.185s | 0.78x |

## Top Regression Targets

- B9 `wc -l` (0.50x)
- B12 `head -100` (0.50x)
- B14 `mean() builtin` (0.61x)
