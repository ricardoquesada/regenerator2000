#!/bin/bash

# Verify that all example projects can regenerate the original binary
# via roundtrip export (export → assemble → diff).

set -e

# Build the project first to avoid repeated build times
cargo build --release

REGEN="./target/release/regenerator2000"

FAILED=0
PASSED=0

run_verify() {
    local project="$1"
    echo "Verifying $project..."
    if $REGEN --headless --verify "$project"; then
        PASSED=$((PASSED + 1))
    else
        FAILED=$((FAILED + 1))
        echo "FAILED: $project"
    fi
    echo ""
}

run_verify examples/c128_hero_is_back.regen2000proj
run_verify examples/c64_burnin_rubber.regen2000proj
#run_verify examples/c64_burnin_rubber_tape_loader.regen2000proj
run_verify examples/c64_moving_tubes_lxt.regen2000proj
run_verify examples/pet_loderunner.regen2000proj
run_verify examples/plus4_kikstart_8192.regen2000proj
run_verify examples/vic20_omega_race_cart_generic_a000.regen2000proj

echo "============================="
echo "Results: $PASSED passed, $FAILED failed"

if [ "$FAILED" -ne 0 ]; then
    echo "Some projects failed verification!"
    exit 1
fi

echo "All projects verified successfully!"
