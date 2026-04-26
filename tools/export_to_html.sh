#!/bin/bash

# Ensure destination directory exists
mkdir -p docs/examples

# Build the project first to avoid repeated build times
cargo build --release

REGEN="./target/release/regenerator2000"

# Export each example
echo "Exporting c64_burnin_rubber..."
$REGEN --headless --export_html docs/examples/c64_burnin_rubber.html examples/c64_burnin_rubber.regen2000proj

#echo "Exporting c64_burnin_rubber_tape_loader..."
#$REGEN --headless --export_html docs/examples/c64_burnin_rubber_tape_loader.html examples/c64_burnin_rubber_tape_loader.regen2000proj

echo "Exporting c64_moving_tubes_lxt..."
$REGEN --headless --export_html docs/examples/c64_moving_tubes_lxt.html examples/c64_moving_tubes_lxt.regen2000proj

echo "Exporting pet_loderunner..."
$REGEN --headless --export_html docs/examples/pet_loderunner.html examples/pet_loderunner.regen2000proj

echo "Exporting plus4_kikstart_8192..."
$REGEN --headless --export_html docs/examples/plus4_kikstart_8192.html examples/plus4_kikstart_8192.regen2000proj

echo "Exporting vic20_omega_race_cart_generic_a000..."
$REGEN --headless --export_html docs/examples/vic20_omega_race_cart_generic_a000.html examples/vic20_omega_race_cart_generic_a000.regen2000proj

echo "All examples exported to docs/examples"
