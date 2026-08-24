#!/bin/sh
set -eu

checkpoint_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checkpoint_root=$(CDPATH= cd -- "$checkpoint_dir/../.." && pwd)
checkpoint_tmp=$(mktemp -d "${TMPDIR:-/tmp}/omega-checkpoint-000001.XXXXXX")
trap 'rm -rf -- "$checkpoint_tmp"' EXIT HUP INT TERM

cd "$checkpoint_root"

python3 compiler/source-checkpoints/verify_manifest.py

cargo run -q -p psi-source-files-to-tokens --bin generate_omega_unicode \
    > "$checkpoint_tmp/unicode_tables.omg"
cmp compiler/psi/generated/unicode_tables.omg "$checkpoint_tmp/unicode_tables.omg"

cargo run -q -p omega-cli -- \
    --build-dir "$checkpoint_tmp/build" apps/omega-compiler/main.omg

checkpoint_program="$checkpoint_tmp/build/omega-program"
if [ ! -x "$checkpoint_program" ]; then
    checkpoint_program="$checkpoint_tmp/build/omega-program.exe"
fi
if [ ! -x "$checkpoint_program" ]; then
    echo "checkpoint 000001: native program was not published" >&2
    exit 1
fi

expect_text_status() {
    label=$1
    expected=$2
    input=$3
    set +e
    printf '%s' "$input" | "$checkpoint_program" >/dev/null 2>&1
    actual=$?
    set -e
    if [ "$actual" -ne "$expected" ]; then
        echo "checkpoint 000001: $label exited $actual, expected $expected" >&2
        exit 1
    fi
}

expect_text_status "empty input" 0 ''
expect_text_status "Unicode identifier" 0 'alpha_π'
expect_text_status "integer" 0 '42'
expect_text_status "punctuation" 0 '::'
expect_text_status "whitespace" 0 '   '
expect_text_status "representative Omega source" 0 'data π { case Zero; }'
expect_text_status "nested block comment" 0 '/* outer /* inner */ end */alpha'
expect_text_status "cooked and raw strings" 0 '"line\n" r#"raw"#'
expect_text_status "unterminated block comment" 251 '/* unterminated'
expect_text_status "invalid cooked-string escape" 251 '"\q"'
expect_text_status "unsupported punctuation" 251 '@'

set +e
printf '\377' | "$checkpoint_program" >/dev/null 2>&1
invalid_utf8_status=$?
set -e
if [ "$invalid_utf8_status" -ne 251 ]; then
    echo "checkpoint 000001: invalid UTF-8 exited $invalid_utf8_status, expected 251" >&2
    exit 1
fi

set +e
dd if=/dev/zero bs=65537 count=1 2>/dev/null \
    | "$checkpoint_program" >/dev/null 2>&1
capacity_status=$?
set -e
if [ "$capacity_status" -ne 252 ]; then
    echo "checkpoint 000001: source overflow exited $capacity_status, expected 252" >&2
    exit 1
fi

echo "checkpoint 000001: manifest, generator, native build, and runtime matrix passed"
