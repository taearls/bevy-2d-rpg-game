# Bevy 2D RPG justfile

# Run the game
run:
    cargo run

# Run the game with the egui debug inspector (right-click a sprite to inspect)
run-debug:
    cargo run --features debug-inspector

# Run with Bevy dynamic linking for the fastest iterative dev builds.
run-fast:
    cargo run --features dynamic_linking

# Build the project (debug configuration)
build:
    cargo build

# Build an optimized release binary
build-release:
    cargo build --release

# Run all tests. Pass `just test verbose` for per-test output.
test verbosity="quiet":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{verbosity}}" = "verbose" ]; then
        cargo test -- --nocapture
    else
        cargo test
    fi

# Auto-fix formatting issues
format:
    cargo fmt

# Check formatting without modifying files
format-check:
    cargo fmt --check

# Lint with clippy, warnings as errors
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Full quality gate: formatting, lints, tests
ci: format-check lint test

# Pin the battle spawn RNG (writes SEED or a random integer to battle.seed).
shuffle SEED="":
    #!/usr/bin/env bash
    set -euo pipefail
    seed="{{SEED}}"
    if [ -z "$seed" ]; then
        # 64-bit unsigned integer — matches the u64 seed range on the Rust side.
        seed=$(od -An -N8 -tu8 < /dev/urandom | tr -d ' \n')
    fi
    echo "$seed" > battle.seed
    echo "Pinned battle seed to $seed (battle.seed)"

# Drop the pinned seed so battles roll fresh RNG each launch again.
unshuffle:
    rm -f battle.seed
    @echo "Removed battle.seed — battles will use fresh RNG each launch."
