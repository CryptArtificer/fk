# fk — filter-kernel
# ──────────────────────────────────────────────────────────────────

CARGO   := cargo
BINARY  := target/release/fk
DEBUG   := target/debug/fk
PREFIX  := /usr/local

# ── Build ────────────────────────────────────────────────────────

.PHONY: build release debug clean

build: release

release:
	$(CARGO) build --release

debug:
	$(CARGO) build

clean:
	$(CARGO) clean
	rm -rf target/ bench_data/

# ── Test ─────────────────────────────────────────────────────────

.PHONY: test test-verbose test-lib test-bin

test:
	$(CARGO) test

test-verbose:
	$(CARGO) test -- --nocapture

test-lib:
	$(CARGO) test --lib

test-bin:
	$(CARGO) test --bin fk

# ── Lint & Format ────────────────────────────────────────────────

.PHONY: check clippy fmt fmt-check lint

check:
	$(CARGO) check

clippy:
	$(CARGO) clippy -- -D warnings

fmt:
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt -- --check

lint: fmt-check clippy

# ── Benchmarks ───────────────────────────────────────────────────

.PHONY: bench bench-field bench-lex bench-record bench-quick

bench:
	$(CARGO) bench

bench-field:
	$(CARGO) bench --bench field_split

bench-lex:
	$(CARGO) bench --bench lex_parse

bench-record:
	$(CARGO) bench --bench record_processing

bench-quick:
	$(CARGO) bench -- --quick

# ── Comparison harness: fk vs awk (vs gawk, mawk if installed) ──

BENCH_DATA  := bench_data
BENCH_LINES := 1000000

.PHONY: bench-compare bench-data

bench-data: $(BENCH_DATA)/large.csv

$(BENCH_DATA)/large.csv:
	@mkdir -p $(BENCH_DATA)
	@echo "Generating $(BENCH_LINES)-line CSV..."
	@awk 'BEGIN { \
		srand(42); \
		for (i = 1; i <= $(BENCH_LINES); i++) \
			printf "%d,%s,%d,%s\n", i, "user_" int(rand()*1000), int(rand()*10000), (rand()>0.5?"active":"inactive") \
	}' > $@
	@wc -l $@ | awk '{ printf "  → %s lines\n", $$1 }'

bench-compare: release bench-data
	@$(CARGO) build --release --features parquet 2>/dev/null || true
	@./scripts/bench-compare.sh $(BINARY) $(BENCH_DATA)/large.csv $(BENCH_LINES)

# ── Install / Uninstall ─────────────────────────────────────────

.PHONY: install uninstall

install: release
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $(BINARY) $(DESTDIR)$(PREFIX)/bin/fk
	install -d $(DESTDIR)$(PREFIX)/share/man/man1
	install -m 644 docs/fk.1 $(DESTDIR)$(PREFIX)/share/man/man1/fk.1

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/fk
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/fk.1

# ── Run shortcuts ────────────────────────────────────────────────

.PHONY: run repl

run: debug
	@echo "Usage: make run ARGS=\"'{ print \$$1 }' file.txt\""
	@if [ -n "$(ARGS)" ]; then $(DEBUG) $(ARGS); fi

repl: debug
	$(DEBUG) --repl

# ── Documentation ────────────────────────────────────────────────

.PHONY: doc doc-open man

doc:
	$(CARGO) doc --no-deps

doc-open:
	$(CARGO) doc --no-deps --open

man:
	@mandoc -Tutf8 docs/fk.1 | less -R

# ── CI-style full check ─────────────────────────────────────────

.PHONY: ci

ci: fmt-check clippy test
	@echo ""
	@echo "✓ All checks passed."

# ── Size report ──────────────────────────────────────────────────

.PHONY: size

size: release
	@echo ""
	@ls -lh $(BINARY) | awk '{ printf "Binary size: %s\n", $$5 }'
	@echo ""
	@echo "Section breakdown (top 10):"
	@if command -v bloat >/dev/null 2>&1; then \
		cargo bloat --release -n 10; \
	elif command -v size >/dev/null 2>&1; then \
		size $(BINARY); \
	else \
		echo "  (install cargo-bloat for detailed breakdown)"; \
	fi

# ── Line count ───────────────────────────────────────────────────

.PHONY: loc

loc:
	@echo ""
	@echo "Lines of code:"
	@if command -v tokei >/dev/null 2>&1; then \
		tokei src/; \
	else \
		find src -name '*.rs' | xargs wc -l | sort -n; \
	fi

# ── Help ─────────────────────────────────────────────────────────

.PHONY: help

help:
	@echo "fk — filter-kernel"
	@echo ""
	@echo "Build:"
	@echo "  make              Build release binary"
	@echo "  make debug        Build debug binary"
	@echo "  make clean        Remove build artifacts"
	@echo ""
	@echo "Test:"
	@echo "  make test         Run all tests"
	@echo "  make test-verbose Run tests with output"
	@echo ""
	@echo "Lint:"
	@echo "  make lint         Format check + clippy"
	@echo "  make fmt          Auto-format code"
	@echo "  make clippy       Run clippy lints"
	@echo ""
	@echo "Bench:"
	@echo "  make bench        Run all criterion benchmarks"
	@echo "  make bench-field  Field splitting benchmarks"
	@echo "  make bench-lex    Lexer/parser benchmarks"
	@echo "  make bench-record Record processing benchmarks"
	@echo "  make bench-compare  fk vs awk head-to-head (1M lines)"
	@echo ""
	@echo "Run:"
	@echo "  make repl         Start interactive REPL"
	@echo "  make run ARGS=..  Run fk with arguments"
	@echo ""
	@echo "Other:"
	@echo "  make ci           Full CI check (fmt, clippy, test)"
	@echo "  make install      Install to $(PREFIX)/bin (+ man page)"
	@echo "  make man          Read the man page"
	@echo "  make size         Binary size report"
	@echo "  make loc          Lines of code"
	@echo "  make doc-open     Generate and open docs"

.DEFAULT_GOAL := help
