#!/bin/sh
set -eu

checkpoint_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checkpoint_root=$(CDPATH= cd -- "$checkpoint_dir/../../../.." && pwd)
checkpoint_tmp=$(mktemp -d "${TMPDIR:-/tmp}/omega-checkpoint-000001.XXXXXX")
trap 'rm -rf -- "$checkpoint_tmp"' EXIT HUP INT TERM

cd "$checkpoint_root"

python3 source/compiler/omega/source-checkpoints/verify_profile.py

cargo run -q --locked --offline -p psi-source-files-to-tokens \
    --bin generate_omega_unicode \
    > "$checkpoint_tmp/unicode_tables.omg"
cmp source/compiler/omega/psi/generated/unicode_tables.omg "$checkpoint_tmp/unicode_tables.omg"

case "$(uname -s):$(uname -m)" in
    Darwin:arm64) checkpoint_target=macos_arm64 ;;
    Linux:x86_64) checkpoint_target=linux_x64 ;;
    Linux:aarch64|Linux:arm64) checkpoint_target=linux_arm64 ;;
    *)
        echo "checkpoint 000001: unsupported native gate host $(uname -s):$(uname -m)" >&2
        exit 2
        ;;
esac

cargo run -q --locked --offline -p omega-cli -- \
    --target "$checkpoint_target" --build-dir "$checkpoint_tmp/build" \
    source/compiler/omega/main.omg

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
    : > "$checkpoint_tmp/runtime.out"
    set +e
    printf '%s' "$input" | "$checkpoint_program" \
        > "$checkpoint_tmp/runtime.out" 2>/dev/null
    actual=$?
    set -e
    if [ "$actual" -ne "$expected" ]; then
        echo "checkpoint 000001: $label exited $actual, expected $expected" >&2
        exit 1
    fi
    if [ -s "$checkpoint_tmp/runtime.out" ]; then
        echo "checkpoint 000001: $label published unexpected stdout" >&2
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
: > "$checkpoint_tmp/runtime.out"
printf '\377' | "$checkpoint_program" > "$checkpoint_tmp/runtime.out" 2>/dev/null
invalid_utf8_status=$?
set -e
if [ "$invalid_utf8_status" -ne 251 ]; then
    echo "checkpoint 000001: invalid UTF-8 exited $invalid_utf8_status, expected 251" >&2
    exit 1
fi
if [ -s "$checkpoint_tmp/runtime.out" ]; then
    echo "checkpoint 000001: invalid UTF-8 published unexpected stdout" >&2
    exit 1
fi

set +e
: > "$checkpoint_tmp/runtime.out"
dd if=/dev/zero bs=65537 count=1 2>/dev/null \
    | "$checkpoint_program" > "$checkpoint_tmp/runtime.out" 2>/dev/null
capacity_status=$?
set -e
if [ "$capacity_status" -ne 252 ]; then
    echo "checkpoint 000001: source overflow exited $capacity_status, expected 252" >&2
    exit 1
fi
if [ -s "$checkpoint_tmp/runtime.out" ]; then
    echo "checkpoint 000001: source overflow published unexpected stdout" >&2
    exit 1
fi

echo "checkpoint 000001: manifest, generator, native build, and runtime matrix passed"
