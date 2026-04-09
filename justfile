# HolyComp developer commands
# Install: cargo install just  |  https://github.com/casey/just

# ── Default ──────────────────────────────────────────────────────────────────
default:
    @just --list

# ── Build ─────────────────────────────────────────────────────────────────────
build:
    cargo build --workspace

release:
    cargo build --workspace --release

# ── Test ──────────────────────────────────────────────────────────────────────
test:
    cargo test --workspace

test-verbose:
    cargo test --workspace -- --nocapture

# Run a single test by name (fuzzy)
test-one NAME:
    cargo test --workspace {{NAME}} -- --nocapture

# ── Lint ─────────────────────────────────────────────────────────────────────
fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# ── Run ──────────────────────────────────────────────────────────────────────
# Start the interactive REPL
repl:
    cargo run -p holyc_interpreter --bin holyc -- repl

# Execute a HolyC source file
run FILE:
    cargo run -p holyc_interpreter --bin holyc -- {{FILE}}

# Run all .HC compatibility tests through the interpreter
compat:
    @echo "=== Running compatibility tests ==="
    @for f in tests/compat/*.HC; do \
        printf "  %-30s" "$f"; \
        if cargo run -q -p holyc_interpreter --bin holyc -- "$f" > /dev/null 2>&1; then \
            echo "PASS"; \
        else \
            echo "FAIL"; \
        fi; \
    done

# Parse a file (compiler front-end only, no execution)
check FILE:
    cargo run -p holyc_compiler --bin holyc-compile -- {{FILE}}

# ── CI (run everything locally) ───────────────────────────────────────────────
ci: fmt-check clippy test compat
    @echo "✓ All CI checks passed"

# ── Utilities ─────────────────────────────────────────────────────────────────
clean:
    cargo clean

# Show all top-level doc comments for a crate
doc CRATE="holyc_frontend":
    cargo doc -p {{CRATE}} --no-deps --open

# Count source lines (requires tokei or scc)
loc:
    @command -v tokei >/dev/null 2>&1 && tokei src || find . -name '*.rs' | xargs wc -l | tail -1

# List all TODO / FIXME comments in source
todos:
    @grep -rn "TODO\|FIXME\|HACK\|XXX" --include="*.rs" . || echo "(none)"
