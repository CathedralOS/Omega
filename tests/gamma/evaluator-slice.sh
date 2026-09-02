#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma evaluator slice: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT
materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator"

make_request() {
    CASE_SOURCE=$1 CASE_INPUT_HEX=$2 python3 -c '
import os, struct, sys
source = os.environ["CASE_SOURCE"].encode("ascii")
invocation_input = bytes.fromhex(os.environ["CASE_INPUT_HEX"])
sys.stdout.buffer.write(b"GAMMAREQ\x01" + struct.pack("<II", len(source), len(invocation_input)) + source + invocation_input)
'
}

assert_case() {
    name=$1
    expected_status=$2
    source=$3
    input_hex=$4
    expected_hex=$5

    make_request "$source" "$input_hex" > "$TMP/request"
    set +e
    "$TMP/evaluator" < "$TMP/request" > "$TMP/output"
    actual_status=$?
    set -e
    if [ "$actual_status" -ne "$expected_status" ]; then
        echo "FAIL: $name status $actual_status, expected $expected_status" >&2
        exit 1
    fi
    EXPECTED_HEX=$expected_hex OUTPUT_PATH="$TMP/output" python3 -c '
import os
from pathlib import Path
actual = Path(os.environ["OUTPUT_PATH"]).read_bytes()
expected = bytes.fromhex(os.environ["EXPECTED_HEX"])
if actual != expected:
    raise SystemExit(f"output {actual.hex()}, expected {expected.hex()}")
' || {
        echo "FAIL: $name output mismatch" >&2
        exit 1
    }
    echo "ok - $name"
}

assert_case identity 0 \
    '(def main (input) (Complete input)) (entry main)' \
    006162ff 006162ff
assert_case forward-entry 0 \
    '(entry main) (def main (input) (Complete #x00ff2a))' \
    '' 00ff2a
assert_case bytes-single 0 \
    '(def main (input) (Complete (bytes-single 65))) (entry main)' \
    '' 41
assert_case bytes-concat 0 \
    '(def main (input) (Complete (bytes-concat #x6162 input))) (entry main)' \
    63 616263
assert_case bytes-length 0 \
    '(def main (input) (Complete (bytes-single (bytes-length input)))) (entry main)' \
    010203 03
assert_case bytes-get 0 \
    '(def main (input) (Complete (bytes-single (bytes-get #x00ff 1)))) (entry main)' \
    '' ff
assert_case bytes-slice 0 \
    '(def main (input) (Complete (bytes-slice (bytes-concat #x6162 input) 1 3))) (entry main)' \
    6364 626364
assert_case bytes-get-bounds 2 \
    '(def main (input) (Complete (bytes-single (bytes-get #x00 1)))) (entry main)' \
    '' ''
assert_case bytes-slice-bounds 2 \
    '(def main (input) (Complete (bytes-slice input 1 1))) (entry main)' \
    '' ''
assert_case arithmetic 0 \
    '(def main (input) (Complete (bytes-single (+ (* 6 7) (- (/ 20 5) (% 8 3)))))) (entry main)' \
    '' 2c
assert_case signed-arithmetic 0 \
    '(def main (input) (Complete (bytes-single (+ -2 3)))) (entry main)' \
    '' 01
assert_case signed-less 0 \
    '(def main (input) (Complete (bytes-single (< (/ -7 2) 0)))) (entry main)' \
    '' 01
assert_case maximum-integer 0 \
    '(def main (input) (if 9223372036854775807 (Complete input) Reject)) (entry main)' \
    61 61
assert_case minimum-integer 0 \
    '(def main (input) (if -9223372036854775808 (Complete input) Reject)) (entry main)' \
    62 62
assert_case positive-literal-overflow 1 \
    '(def main (input) (Complete 9223372036854775808)) (entry main)' \
    '' ''
assert_case negative-literal-overflow 1 \
    '(def main (input) (Complete -9223372036854775809)) (entry main)' \
    '' ''
assert_case add-overflow 2 \
    '(def main (input) (Complete (bytes-single (+ 9223372036854775807 1)))) (entry main)' \
    '' ''
assert_case subtract-overflow 2 \
    '(def main (input) (Complete (bytes-single (- -9223372036854775808 1)))) (entry main)' \
    '' ''
assert_case multiply-overflow 2 \
    '(def main (input) (Complete (bytes-single (* 9223372036854775807 2)))) (entry main)' \
    '' ''
assert_case divide-zero 2 \
    '(def main (input) (Complete (bytes-single (/ 1 0)))) (entry main)' \
    '' ''
assert_case divide-overflow 2 \
    '(def main (input) (Complete (bytes-single (/ -9223372036854775808 -1)))) (entry main)' \
    '' ''
assert_case remainder-overflow 2 \
    '(def main (input) (Complete (bytes-single (% -9223372036854775808 -1)))) (entry main)' \
    '' ''
assert_case nested-let 0 \
    '(def main (input) (let x 65 (let y 1 (Complete (bytes-single (+ x y)))))) (entry main)' \
    '' 42
assert_case bytes-let 0 \
    '(def main (input) (let prefix #x6162 (Complete (bytes-concat prefix input)))) (entry main)' \
    63 616263
assert_case duplicate-parameter-binding 2 \
    '(def main (input) (let input #x (Complete input))) (entry main)' \
    '' ''
assert_case duplicate-active-let 2 \
    '(def main (input) (let x 1 (let x 2 (Complete #x)))) (entry main)' \
    '' ''
assert_case unreachable-duplicate-let 0 \
    '(def main (input) (if 0 (let input #x Reject) (Complete input))) (entry main)' \
    61 61
assert_case integer-equality 0 \
    '(def main (input) (if (= (+ 1 2) 3) (Complete input) Reject)) (entry main)' \
    61 61
assert_case cross-kind-equality 0 \
    '(def main (input) (if (= 1 #x01) Reject (Complete input))) (entry main)' \
    62 62
assert_case logical-bytes-equality 0 \
    '(def main (input) (if (= (bytes-slice #x006162ff 1 2) (bytes-concat #x61 #x62)) (Complete input) Reject)) (entry main)' \
    63 63
assert_case unequal-bytes 0 \
    '(def main (input) (if (= #x6162 #x6163) Reject (Complete input))) (entry main)' \
    64 64
assert_case reject-equality 0 \
    '(def main (input) (if (= Reject Reject) (Complete input) Reject)) (entry main)' \
    65 65
assert_case complete-equality 0 \
    '(def main (input) (if (= (Complete #x61) (Complete #x61)) (Complete input) Reject)) (entry main)' \
    66 66
assert_case if-true 0 \
    '(def main (input) (if 1 (Complete input) (Complete 2))) (entry main)' \
    61 61
assert_case if-false 0 \
    '(def main (input) (if 0 (Complete 1) (Complete input))) (entry main)' \
    62 62
assert_case authored-reject 2 \
    '(def main (input) Reject) (entry main)' \
    '' ''
assert_case runtime-kind-trap 2 \
    '(def main (input) (Complete 1)) (entry main)' \
    '' ''
assert_case malformed-source 1 \
    '(def main (input) (Complete #x0)) (entry main)' \
    '' ''
assert_case unresolved-entry 1 \
    '(def main (input) (Complete input)) (entry other)' \
    '' ''
assert_case unresolved-call 2 \
    '(def main (input) (helper input)) (entry main)' \
    '' ''
assert_case unused-declaration 0 \
    '(def helper (value) (Complete #x00)) (def main (input) (Complete input)) (entry main)' \
    61 61
assert_case later-entry 0 \
    '(def first (value) (Complete #x61)) (entry second) (def second (input) (Complete #x62))' \
    '' 62
assert_case duplicate-function 1 \
    '(def same (value) (Complete #x61)) (def same (input) (Complete #x62)) (entry same)' \
    '' ''
assert_case forward-call 0 \
    '(def main (input) (helper input)) (def helper (value) (Complete value)) (entry main)' \
    61 61
assert_case nested-call 0 \
    '(def addone (value) (+ value 1)) (def main (input) (Complete (bytes-single (addone 64)))) (entry main)' \
    '' 41
assert_case lexical-isolation 0 \
    '(def keep (value) (let x 66 (bytes-single value))) (def main (input) (let x 65 (Complete (keep x)))) (entry main)' \
    '' 41
assert_case mutual-call 0 \
    '(def even (value) (if (= value 0) 1 (odd (- value 1)))) (def odd (value) (if (= value 0) 0 (even (- value 1)))) (def main (input) (Complete (bytes-single (even 20)))) (entry main)' \
    '' 01
assert_case unreachable-unresolved-call 0 \
    '(def main (input) (if 0 (missing input) (Complete input))) (entry main)' \
    62 62
assert_case deep-bounded-recursion 0 \
    '(def loop (value) (if (= value 0) (Complete #x01) (loop (- value 1)))) (def main (input) (loop 10000)) (entry main)' \
    '' 01
assert_case pair-first 0 \
    '(def main (input) (Complete (pair-first (pair input 7)))) (entry main)' \
    6162 6162
assert_case pair-second 0 \
    '(def main (input) (Complete (bytes-single (pair-second (pair input 65))))) (entry main)' \
    '' 41
assert_case nested-pair 0 \
    '(def main (input) (Complete (pair-first (pair (pair-second (pair 0 input)) 1)))) (entry main)' \
    63 63
assert_case pair-through-call 0 \
    '(def select (value) (pair-second value)) (def main (input) (Complete (select (pair 0 input)))) (entry main)' \
    64 64
assert_case pair-kind-trap 2 \
    '(def main (input) (Complete (pair-first input))) (entry main)' \
    65 ''
assert_case pair-strict 2 \
    '(def main (input) (Complete (pair-first (pair (/ 1 0) input)))) (entry main)' \
    66 ''
assert_case pair-equality-trap 2 \
    '(def main (input) (if (= (pair 1 2) (pair 1 2)) (Complete input) Reject)) (entry main)' \
    67 ''
assert_case pair-cross-equality-trap 2 \
    '(def main (input) (if (= (pair 1 2) 1) (Complete input) Reject)) (entry main)' \
    68 ''
assert_case uppercase-identifier 1 \
    '(def main (input) (if 0 Uppercase (Complete input))) (entry main)' \
    69 ''
assert_case retired-data-form 1 \
    '(data Maybe (None 0) (Some 1)) (def main (input) (Complete input)) (entry main)' \
    6a ''
assert_case retired-match-form 1 \
    '(def main (input) (match input (_ (Complete input)))) (entry main)' \
    6b ''

printf 'BAD' > "$TMP/request"
set +e
"$TMP/evaluator" < "$TMP/request" > "$TMP/output"
actual_status=$?
set -e
[ "$actual_status" -eq 1 ] && [ ! -s "$TMP/output" ] || {
    echo "FAIL: malformed request was not a quiet status-1 result" >&2
    exit 1
}
echo "ok - malformed-request"

echo "Gamma evaluator core: 62/62 cases passed"