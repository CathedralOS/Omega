#!/usr/bin/env sh
# Shared typed terminal-codec primitive contract.
set -eu
cd "$(dirname "$0")"
. ../alpha/seed_env.sh

SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || {
  echo "terminal codec primitives: bc build failed" >&2
  exit 1
}

build_beta_program() {
  source=$1
  output=$2
  ../beta-lang-rs/build/bc.exe < "$source" > "$T/program.asm"
  "$ASM" < "$T/program.asm" > "$T/program.tape"
  stamp_seed "$T/program.tape" "$SEED" "$output" >/dev/null 2>&1
}

build_beta_program typeck.beta "$T/typeck.exe"
build_beta_program interp.beta "$T/interp.exe"

cat canonical-bytes/types.gamma \
    terminal-codec-primitives/types.gamma \
    canonical-bytes/decode.gamma \
    terminal-codec-primitives/header.gamma \
    terminal-codec-primitives/scalars.gamma \
    terminal-codec-primitives/scalar_types.gamma \
    terminal-codec-primitives/utf8.gamma \
    terminal-codec-primitives/tests.gamma > "$T/typed.gamma"

set +e
"$T/typeck.exe" < "$T/typed.gamma"
type_status=$?
set -e
if [ "$type_status" != 1 ]; then
  echo "terminal codec primitives: typed Gamma program was rejected (status $type_status)" >&2
  exit 1
fi

python3 erase_types.py < "$T/typed.gamma" > "$T/run.gamma"
printf '\n(terminal_codec_primitives_self_test)\n' >> "$T/run.gamma"

set +e
beta_output=$("$T/interp.exe" < "$T/run.gamma")
beta_status=$?
python_output=$(python3 gamma_ref.py < "$T/run.gamma")
python_status=$?
set -e

if [ "$beta_status" != 1 ] || [ "$beta_output" != 1 ]; then
  echo "terminal codec primitives: Beta result was '$beta_output'/$beta_status, expected 1" >&2
  exit 1
fi
if [ "$python_status" != 1 ] || [ "$python_output" != 1 ]; then
  echo "terminal codec primitives: Python result was '$python_output'/$python_status, expected 1" >&2
  exit 1
fi

echo "terminal codec primitives: current header/scalar/type/UTF-8 contract -> 1 (Beta/Python agree)"
