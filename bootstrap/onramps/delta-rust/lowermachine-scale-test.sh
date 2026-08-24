#!/usr/bin/env sh
# Regression for lowermachine's aggregate machine-parameter tables.  The D0
# calling profile permits four value parameters per free machine and three per
# self method; with 128 machines, the four parallel metadata columns therefore
# need 512 checked rows.  A former 64-row partition let parameter 65 overwrite
# the first field-name row even though every individual signature was valid.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
SAMPLES="$HERE/../../rungs/delta/samples"
BIN="$HERE/target/debug/delta"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "lowermachine scale: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for tool in cargo clang codesign; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "lowermachine scale: skipped ($tool absent)"
    exit 0
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT HUP INT TERM

cd "$HERE"
cargo build --quiet
DELTA_ARCH=aarch64 "$BIN" "$SAMPLES/lowermachine.alp" "$T/lowermachine" >/dev/null

make_aggregate_parameter_source() {
  output=$1
  printf '%s' 'boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; result: i32; } ' > "$output"
  helper=0
  while [ "$helper" -lt 65 ]; do
    printf 'machine helper%s(n: i32) -> i32 { return n; } ' "$helper" >> "$output"
    helper=$((helper + 1))
  done
  printf '%s' 'machine Main::main(&mut self) { self.result = helper64(37); self.console.exit_process(self.result); }' >> "$output"
}

make_aggregate_parameter_source "$T/aggregate-parameters.alp"
DELTA_ARCH=aarch64 "$BIN" "$T/aggregate-parameters.alp" "$T/reference" >/dev/null
"$T/lowermachine" < "$T/aggregate-parameters.alp" > "$T/self.s"

cmp "$T/reference.s" "$T/self.s" >/dev/null
grep -q '^Lmachine64:$' "$T/self.s"
grep -q '^    bl Lmachine64$' "$T/self.s"
clang -arch arm64 -o "$T/self" "$T/self.s"
codesign -f -s - "$T/self" >/dev/null 2>&1

set +e
"$T/reference" >/dev/null 2>&1
reference_status=$?
"$T/self" >/dev/null 2>&1
self_status=$?
set -e
if [ "$reference_status" -ne 37 ] || [ "$self_status" -ne 37 ]; then
  echo "lowermachine aggregate-parameter FAIL — reference=$reference_status self=$self_status, expected 37"
  exit 1
fi

# Delta's keywords are contextual: `machine` may name a field or parameter,
# while `data()` may name a state target inside a machine body.
# Declaration pre-scans must therefore recognize a top-level item structurally,
# not treat every same-spelled token before a machine body as another machine.
printf '%s' 'boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; machine: i32; result: i32; } machine echo(machine: i32) -> i32 { transition 0 { _ -> data() } state data() { return machine; } } machine helper(n: i32) -> i32 { return n + 32; } machine Main::main(&mut self) { self.machine = 5; self.result = echo(self.machine); self.result = helper(self.result); self.console.exit_process(self.result); }' > "$T/contextual-machine.alp"
DELTA_ARCH=aarch64 "$BIN" "$T/contextual-machine.alp" "$T/contextual-reference" >/dev/null
"$T/lowermachine" < "$T/contextual-machine.alp" > "$T/contextual-self.s"
cmp "$T/contextual-reference.s" "$T/contextual-self.s" >/dev/null
clang -arch arm64 -o "$T/contextual-self" "$T/contextual-self.s"
codesign -f -s - "$T/contextual-self" >/dev/null 2>&1
set +e
"$T/contextual-reference" >/dev/null 2>&1
contextual_reference_status=$?
"$T/contextual-self" >/dev/null 2>&1
contextual_self_status=$?
set -e
if [ "$contextual_reference_status" -ne 37 ] || [ "$contextual_self_status" -ne 37 ]; then
  echo "lowermachine contextual-machine FAIL — reference=$contextual_reference_status self=$contextual_self_status, expected 37"
  exit 1
fi

# A fifth free-machine value parameter exceeds the D0 calling profile.  The
# compiler must reject before phase 2 publishes any assembly.
printf '%s' 'boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; } machine too_many(a: i32, b: i32, c: i32, d: i32, e: i32) { return 0; } machine Main::main(&mut self) { self.console.exit_process(0); }' > "$T/too-many.alp"
set +e
"$T/lowermachine" < "$T/too-many.alp" > "$T/too-many.s" 2>/dev/null
too_many_status=$?
set -e
if [ "$too_many_status" -ne 3 ] || [ -s "$T/too-many.s" ]; then
  echo "lowermachine parameter-bound FAIL — status=$too_many_status, expected 3 with no output"
  exit 1
fi

echo "LOWERMACHINE SCALE ✓ — parameter 65 stays disjoint; contextual machine/data identifiers remain non-declarations; D0 signature bounds fail closed"
