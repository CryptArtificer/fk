# Performance Baseline (Strict)

Baseline captured with `tests/suite/perf_strict.sh` (warmup=1, reps=9).
Report file: `bench_data/perf_strict_1000000_20260219-112213.txt`.

## fk vs awk (1,000,000 lines)

| Benchmark | awk median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| print $2 | 0.610s | 0.138s | 4.43x |
| sum column | 0.560s | 0.213s | 2.63x |
| pattern match | 0.725s | 0.200s | 3.62x |
| field arithmetic | 0.873s | 0.346s | 2.52x |
| associative array | 0.646s | 0.264s | 2.44x |
| frequency count | 0.266s | 0.235s | 1.13x |
| gsub | 0.988s | 0.422s | 2.34x |
| NR==FNR join | 1.517s | 0.486s | 3.12x |

## fk vs tools (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| wc -l | 0.046s | 0.090s | 0.51x |
| grep pattern | 0.182s | 0.158s | 1.15x |
| cut -d' ' -f2 | 0.273s | 0.138s | 1.98x |
| head -100 | 0.022s | 0.039s | 0.57x |
| uniq (sorted input) | 0.161s | 0.235s | 0.69x |

## fk-only features (1,000,000 lines)

| Benchmark | tool median | fk median | Ratio (tool/fk) |
|---|---:|---:|---:|
| mean() builtin | 0.229s | 0.518s | 0.44x |
| median() builtin | 1.068s | 0.514s | 2.08x |
| reverse() builtin | 0.147s | 0.187s | 0.79x |

## Top Regression Targets

- B14 `mean() builtin` (0.44x)
- B9 `wc -l` (0.51x)
- B12 `head -100` (0.57x)
