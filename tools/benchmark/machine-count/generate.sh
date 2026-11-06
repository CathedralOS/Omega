#!/bin/sh
# Writes a program whose compile cost should grow linearly with machine count:
# the one-line `Main::main` plus N extra machines of the same shape. Every
# extra machine is checked in full even though nothing calls it, so the
# per-machine increment of `omega --check` (or of one phase-profile scope)
# exposes any check whose cost depends on the size of the whole program.
#
# Usage: sh tools/benchmark/machine-count/generate.sh N DIR
#   then: OMEGA_PHASE_PROFILE=1 omega --check --timings DIR/main.omg < /dev/null
set -eu
n=$1
dir=$2
mkdir -p "$dir"
printf 'machine build(builder: &mut Build) {\n    Build::application(builder, "machine_count");\n    builder.freestanding = true;\n}\n' > "$dir/build.omg"
{
  printf 'data Main {\n    marker: u64;\n}\nmachine Main::main(main: &mut Main) {\n    main.marker = 0;\n}\n'
  i=1
  while [ "$i" -le "$n" ]; do
    printf 'machine Main::step_%d(main: &mut Main) {\n    main.marker = %d;\n}\n' "$i" "$i"
    i=$((i + 1))
  done
} > "$dir/main.omg"
