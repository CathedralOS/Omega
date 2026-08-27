#!/bin/sh
set -eu

checkpoint_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checkpoint_root=$(CDPATH= cd -- "$checkpoint_dir/../../.." && pwd)
checkpoint_tmp=$(mktemp -d "${TMPDIR:-/tmp}/omega-checkpoint-000001.XXXXXX")
trap 'rm -rf -- "$checkpoint_tmp"' EXIT HUP INT TERM

cd "$checkpoint_root"

python3 source/omega/source-checkpoints/verify_profile.py

cargo run -q --locked --offline -p psi-source-files-to-tokens \
    --bin generate_omega_unicode \
    > "$checkpoint_tmp/unicode_tables.omg"
cmp source/psi/generated/unicode_tables.omg "$checkpoint_tmp/unicode_tables.omg"

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
    --target "$checkpoint_target" --build-dir "$checkpoint_tmp/build" --output-only \
    source/omega/main.omg

checkpoint_program="$checkpoint_tmp/build/omega-program"
if [ ! -x "$checkpoint_program" ]; then
    checkpoint_program="$checkpoint_tmp/build/omega-program.exe"
fi
if [ ! -x "$checkpoint_program" ]; then
    echo "checkpoint 000001: native program was not published" >&2
    exit 1
fi

compare_input_observation() {
    label=$1
    expected=$2
    input_file=$3
    : > "$checkpoint_tmp/product.observation"
    set +e
    "$checkpoint_program" < "$input_file" \
        > "$checkpoint_tmp/product.observation" 2>/dev/null
    actual=$?
    set -e
    if [ "$actual" -ne "$expected" ]; then
        echo "checkpoint 000001: $label exited $actual, expected $expected" >&2
        exit 1
    fi

    cargo run -q --locked --offline -p psi-source-files-to-tokens \
        --bin observe_omega_lexer \
        < "$input_file" > "$checkpoint_tmp/rust.observation"
    if ! cmp "$checkpoint_tmp/product.observation" "$checkpoint_tmp/rust.observation"; then
        echo "checkpoint 000001: $label lexical observation mismatch" >&2
        exit 1
    fi
}

compare_text_observation() {
    label=$1
    expected=$2
    input=$3
    printf '%s' "$input" > "$checkpoint_tmp/input"
    compare_input_observation "$label" "$expected" "$checkpoint_tmp/input"
}

compare_text_observation "empty input" 0 ''
compare_text_observation "Unicode identifier" 0 'alpha_π'
compare_text_observation "integer" 0 '42'
compare_text_observation "punctuation" 0 '::'
compare_text_observation "whitespace" 0 '   '
compare_text_observation "representative Omega source" 0 \
    'data π { case Zero; } "line\n" r#"raw"#'
compare_text_observation "nested block comment" 0 \
    '/* outer /* inner */ end */alpha'
compare_text_observation "lexical rejection with retained prefix" 251 'alpha @'
compare_text_observation "unterminated block comment" 251 '/* unterminated'
compare_text_observation "invalid cooked-string escape" 251 '"\q"'

printf 'ok\377tail' > "$checkpoint_tmp/input"
compare_input_observation "invalid UTF-8" 251 "$checkpoint_tmp/input"

awk 'BEGIN { for (i = 0; i < 16385; i++) printf "; " }' \
    > "$checkpoint_tmp/input"
compare_input_observation "token capacity" 251 "$checkpoint_tmp/input"

dd if=/dev/zero of="$checkpoint_tmp/input" bs=65537 count=1 2>/dev/null
compare_input_observation "source capacity" 252 "$checkpoint_tmp/input"

cp "$checkpoint_tmp/product.observation" "$checkpoint_tmp/tampered.observation"
printf '\000' | dd of="$checkpoint_tmp/tampered.observation" \
    bs=1 seek=0 conv=notrunc 2>/dev/null
if cmp -s "$checkpoint_tmp/tampered.observation" "$checkpoint_tmp/rust.observation"; then
    echo "checkpoint 000001: tampered lexical observation was accepted" >&2
    exit 1
fi

echo "checkpoint 000001: manifest, generator, native build, and differential lexical observation matrix passed"
