#!/usr/bin/env sh
# Focused native acceptance gate for the Delta-written O0/O1 front end.
set -e
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "omega-bootstrap frontend: repository root not found" >&2; exit 2; }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
cd "$GATE_DIR"
SAMPLES="$OMEGA_PATH_DELTA/samples"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "omega-bootstrap frontend: native gate skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v cargo >/dev/null 2>&1 || { echo "omega-bootstrap frontend: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 || { echo "omega-bootstrap frontend: skipped (python3 absent)"; exit 0; }
command -v clang >/dev/null 2>&1 || { echo "omega-bootstrap frontend: skipped (clang absent)"; exit 0; }
command -v codesign >/dev/null 2>&1 || { echo "omega-bootstrap frontend: skipped (codesign absent)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cargo build -q
FRONTEND="$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega-bootstrap-frontend.alp"
DELTA_ARCH=aarch64 ./target/debug/delta "$FRONTEND" "$T/frontend" >/dev/null

PASS=0
FAIL=0
bundle_one() {
  python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack main.omg="$1" > "$2"
}
run_bundle() {
  label=$1 input=$2 expected=$3
  set +e
  "$T/frontend" < "$input" > /dev/null 2>&1
  got=$?
  set -e
  if [ "$got" = "$expected" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); echo "  FAIL $label: exit $got, expected $expected"
  fi
}
run_bundle_empty() {
  label=$1 input=$2 expected=$3
  set +e
  "$T/frontend" < "$input" > "$T/rejected.terminal" 2>/dev/null
  got=$?
  set -e
  if [ "$got" = "$expected" ] && [ ! -s "$T/rejected.terminal" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); echo "  FAIL $label: exit $got, expected $expected; rejection published bytes"
  fi
}
run_source() {
  label=$1 source=$2 expected=$3
  bundle_one "$source" "$T/case.bundle"
  run_bundle "$label" "$T/case.bundle" "$expected"
}

# The shared product lowerer owns terminal-Psi canonicality. Export the exact
# 0/1/2/16-write references, then require the Delta emitter to agree byte for
# byte rather than validating only its success digest.
REFERENCE_DIR="$T/o1-terminal-references"
(
  cd "$OMEGA_REPO_ROOT"
  OMEGA1_WRITE_TERMINAL_REFERENCES="$REFERENCE_DIR" \
    cargo test -q -p psi-checked-trees-to-terminal \
      --test provider_attachment_source \
      straight_line_console_projection_accepts_zero_one_two_and_sixteen_writes -- --exact
)
reference_case() {
  count=$1 expected=$2
  {
    printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
    index=0
    while [ "$index" -lt "$count" ]; do
      printf 'self.console.write_line("line-%02d");' "$index"
      index=$((index + 1))
    done
    printf 'self.console.exit_process(%d);}' "$count"
  } > "$T/reference.omg"
  bundle_one "$T/reference.omg" "$T/reference.bundle"
  set +e
  "$T/frontend" < "$T/reference.bundle" > "$T/reference.terminal"
  got=$?
  set -e
  if [ "$got" = "$expected" ] \
      && cmp -s "$REFERENCE_DIR/writes-$count.terminal" "$T/reference.terminal"; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "  FAIL O1 shared terminal reference writes=$count: exit $got, expected $expected"
  fi
}
reference_case 0 0
reference_case 1 64
reference_case 2 129
reference_case 16 86

run_source "canonical cli_mvp" "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" 107

# The complete bundle is decoded before selecting exactly one program unit.
# Its label may sort after auxiliary units; empty, line-comment-only, and nested
# block-comment-only units remain independent and cannot fuse tokens with the
# program.
: > "$T/empty.omg"
printf '// auxiliary /* remains line text without final newline' > "$T/comment.omg"
printf '/* outer auxiliary // remains block text /* nested */ tail */' > "$T/block-comment.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/empty.omg="$T/empty.omg" b/block.omg="$T/block-comment.omg" \
  m/comment.omg="$T/comment.omg" \
  z/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" > "$T/auxiliary.bundle"
bundle_one "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" "$T/canonical.bundle"
set +e
"$T/frontend" < "$T/canonical.bundle" > "$T/canonical.terminal"
canonical_status=$?
"$T/frontend" < "$T/auxiliary.bundle" > "$T/auxiliary.terminal"
auxiliary_status=$?
set -e
if [ "$canonical_status" = 107 ] && [ "$auxiliary_status" = 107 ] \
    && cmp -s "$T/canonical.terminal" "$T/auxiliary.terminal"; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1)); echo "  FAIL multi-source auxiliary trivia: canonical=$canonical_status auxiliary=$auxiliary_status"
fi

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a00.omg="$T/empty.omg" a01.omg="$T/empty.omg" a02.omg="$T/empty.omg" \
  a03.omg="$T/empty.omg" a04.omg="$T/empty.omg" a05.omg="$T/empty.omg" \
  a06.omg="$T/empty.omg" a07.omg="$T/empty.omg" a08.omg="$T/empty.omg" \
  a09.omg="$T/empty.omg" a10.omg="$T/empty.omg" a11.omg="$T/empty.omg" \
  a12.omg="$T/empty.omg" a13.omg="$T/empty.omg" a14.omg="$T/empty.omg" \
  z/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" > "$T/descriptor-full.bundle"
set +e
"$T/frontend" < "$T/descriptor-full.bundle" > "$T/descriptor-full.terminal"
descriptor_full_status=$?
set -e
if [ "$descriptor_full_status" = 107 ] \
    && cmp -s "$T/canonical.terminal" "$T/descriptor-full.terminal"; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1)); echo "  FAIL exact 16-source descriptor ceiling: status=$descriptor_full_status"
fi

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.exit_process(7);}' > "$T/zero-write.omg"
run_source "O1 zero writes" "$T/zero-write.omg" 7

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("A");self.console.write_line("BC");self.console.exit_process(3);}' > "$T/two-write.omg"
run_source "O1 two ordered writes" "$T/two-write.omg" 201

{
  printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
  write=0
  while [ "$write" -lt 16 ]; do
    printf 'self.console.write_line("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");'
    write=$((write + 1))
  done
  printf 'self.console.exit_process(0);}'
} > "$T/sixteen-write.omg"
run_source "O1 sixteen writes and exact aggregate text ceiling" "$T/sixteen-write.omg" 141

printf 'use omega::language::std::console; // import\ndata Main{console:Console;}machine Main::main(&mut self){self.console.write_line("A\\n");self.console.exit_process(2);}' > "$T/variant.omg"
run_source "trivia, cooked escape, no final newline" "$T/variant.omg" 77

printf '/* before */use/* outer /* nested */ tail */omega::language::std::console;data Main{/* field */console:Console;}machine Main::main(&mut self){self.console.write_line("A/* literal */");/* body */self.console.exit_process(2);}' > "$T/block-program.omg"
bundle_one "$T/block-program.omg" "$T/block-program.bundle"
set +e
"$T/frontend" < "$T/block-program.bundle" > "$T/block-program.terminal"
block_program_status=$?
set -e
if [ "$block_program_status" = 54 ]; then
  PASS=$((PASS+1))
else
  FAIL=$((FAIL+1)); echo "  FAIL nested block comments in program: status=$block_program_status"
fi

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("\303\251");self.console.exit_process(3);}' > "$T/utf8.omg"
run_source "UTF-8 string payload" "$T/utf8.omg" 116

reject_source() {
  label=$1 body=$2
  printf '%s' "$body" > "$T/reject.omg"
  run_source "$label" "$T/reject.omg" 251
}
reject_source_empty() {
  label=$1 body=$2
  printf '%s' "$body" > "$T/reject-empty.omg"
  bundle_one "$T/reject-empty.omg" "$T/reject-empty.bundle"
  run_bundle_empty "$label" "$T/reject-empty.bundle" 251
}

reject_source "unknown import" 'use omega::language::std::other; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "duplicate declaration" 'use omega::language::std::console; data Main{console:Console;} data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "missing entry" 'use omega::language::std::console; data Main{console:Console;}'
reject_source "wrong field type" 'use omega::language::std::console; data Main{console:Main;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "wrong machine receiver" 'use omega::language::std::console; data Main{console:Console;} machine Console::main(&mut self){self.console.write_line("x");self.console.exit_process(0);}'
reject_source "unknown operation" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write("x");self.console.exit_process(0);}'
reject_source "write_line missing argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line();self.console.exit_process(0);}'
reject_source "write_line extra argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x","y");self.console.exit_process(0);}'
reject_source "write_line wrong argument type" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line(1);self.console.exit_process(0);}'
reject_source "exit_process missing argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process();}'
reject_source "exit_process extra argument" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0,1);}'
reject_source "exit_process wrong argument type" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process("0");}'
reject_source "reversed effects" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.exit_process(0);self.console.write_line("x");}'
reject_source "duplicate exit" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.exit_process(0);self.console.exit_process(1);}'
reject_source "trailing construct" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(0);} data Extra{}'
reject_source "unterminated string" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x);self.console.exit_process(0);}'
reject_source "invalid escape" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("\q");self.console.exit_process(0);}'
reject_source "i32 literal overflow" 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("x");self.console.exit_process(2147483648);}'
reject_source_empty "unterminated nested block comment in program" 'use omega::language::std::console; data Main{console:Console;} /* outer /* nested */ machine Main::main(&mut self){self.console.exit_process(0);}'

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("' > "$T/invalid-utf8.omg"
printf '\377' >> "$T/invalid-utf8.omg"
printf '");self.console.exit_process(0);}' >> "$T/invalid-utf8.omg"
run_source "invalid UTF-8" "$T/invalid-utf8.omg" 251

cp "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" "$T/nul-tail.omg"
printf '\000ignored' >> "$T/nul-tail.omg"
run_source "raw NUL cannot masquerade as source EOF" "$T/nul-tail.omg" 251

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a.omg="$T/variant.omg" b.omg="$T/variant.omg" > "$T/multi.bundle"
run_bundle_empty "two program-bearing sources" "$T/multi.bundle" 251

printf 'use omega::language::std::' > "$T/token-left.omg"
printf 'console;' > "$T/token-right.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a.omg="$T/token-left.omg" b.omg="$T/token-right.omg" > "$T/token-fragments.bundle"
run_bundle_empty "cross-source token fragments" "$T/token-fragments.bundle" 251

printf '\377' > "$T/invalid-auxiliary.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/invalid.omg="$T/invalid-auxiliary.omg" z/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" \
  > "$T/invalid-auxiliary.bundle"
run_bundle_empty "invalid UTF-8 in auxiliary source" "$T/invalid-auxiliary.bundle" 251

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" z/invalid.omg="$T/invalid-auxiliary.omg" \
  > "$T/invalid-auxiliary-after.bundle"
run_bundle_empty "invalid UTF-8 after program source" "$T/invalid-auxiliary-after.bundle" 251

printf '/* unterminated auxiliary' > "$T/unterminated-comment.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/comment.omg="$T/unterminated-comment.omg" z/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" \
  > "$T/unterminated-comment.bundle"
run_bundle_empty "unterminated auxiliary block comment" "$T/unterminated-comment.bundle" 251

printf '/*' > "$T/comment-open.omg"
printf '*/' > "$T/comment-close.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/open.omg="$T/comment-open.omg" b/close.omg="$T/comment-close.omg" \
  z/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" > "$T/cross-unit-comment.bundle"
run_bundle_empty "block comment cannot close across source units" "$T/cross-unit-comment.bundle" 251

python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/empty.omg="$T/empty.omg" b/comment.omg="$T/comment.omg" > "$T/all-trivia.bundle"
run_bundle_empty "bundle without a program-bearing source" "$T/all-trivia.bundle" 251

dd if=/dev/zero of="$T/oversize.omg" bs=2049 count=1 2>/dev/null
run_source "checked source exhaustion" "$T/oversize.omg" 252

# Aggregate source backing is shared across descriptors. Force exhaustion while
# decoding the second entry so a multi-source bundle cannot be rejected merely
# from count > 1.
dd if=/dev/zero bs=1 count=1500 2>/dev/null | tr '\000' ' ' > "$T/large-trivia.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/source/omega_bootstrap_bundle.py" pack \
  a/trivia.omg="$T/large-trivia.omg" z/program.omg="$OMEGA_PATH_CORPUS/cli_mvp/main.omg" \
  > "$T/aggregate-source-exhaust.bundle"
run_bundle_empty "checked aggregate source exhaustion" "$T/aggregate-source-exhaust.bundle" 252

# The descriptor and per-label ceilings are explicit resource outcomes.
python3 - "$T/descriptor-exhaust.bundle" "$T/label-exhaust.bundle" \
  "$T/descending.bundle" "$T/duplicate.bundle" "$T/unsafe-label.bundle" \
  "$T/high-u32.bundle" "$T/truncated-u32.bundle" <<'PY'
import pathlib
import struct
import sys

magic = b"OMG0BNDL"
pathlib.Path(sys.argv[1]).write_bytes(struct.pack("<8sII", magic, 1, 17))
label = b"a" * 65
pathlib.Path(sys.argv[2]).write_bytes(
    struct.pack("<8sIIII", magic, 1, 1, len(label), 0) + label
)

def bundle(entries):
    data = bytearray(struct.pack("<8sII", magic, 1, len(entries)))
    for label, content in entries:
        data.extend(struct.pack("<II", len(label), len(content)))
        data.extend(label)
        data.extend(content)
    return bytes(data)

pathlib.Path(sys.argv[3]).write_bytes(bundle([(b"z.omg", b""), (b"a.omg", b"")]))
pathlib.Path(sys.argv[4]).write_bytes(bundle([(b"a.omg", b""), (b"a.omg", b"")]))
pathlib.Path(sys.argv[5]).write_bytes(bundle([(b"../bad.omg", b"")]))
pathlib.Path(sys.argv[6]).write_bytes(magic + struct.pack("<I", 0x80000001))
pathlib.Path(sys.argv[7]).write_bytes(magic + b"\x01\x00")
PY
run_bundle_empty "checked descriptor exhaustion" "$T/descriptor-exhaust.bundle" 252
run_bundle_empty "checked label exhaustion" "$T/label-exhaust.bundle" 252
run_bundle_empty "descending labels reject" "$T/descending.bundle" 251
run_bundle_empty "duplicate labels reject" "$T/duplicate.bundle" 251
run_bundle_empty "unsafe label rejects" "$T/unsafe-label.bundle" 251
run_bundle_empty "high-bit u32 rejects" "$T/high-u32.bundle" 251
run_bundle_empty "truncated u32 rejects" "$T/truncated-u32.bundle" 251

printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){self.console.write_line("' > "$T/text-exhaust.omg"
dd if=/dev/zero bs=1 count=1025 2>/dev/null | tr '\000' x >> "$T/text-exhaust.omg"
printf '");self.console.exit_process(0);}' >> "$T/text-exhaust.omg"
run_source "checked decoded-string exhaustion" "$T/text-exhaust.omg" 252

{
  printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
  write=0
  while [ "$write" -lt 17 ]; do
    printf 'self.console.write_line("");'
    write=$((write + 1))
  done
  printf 'self.console.exit_process(0);}'
} > "$T/write-table-exhaust.omg"
run_source "checked write table exhaustion" "$T/write-table-exhaust.omg" 252

{
  printf 'use omega::language::std::console; data Main{console:Console;} machine Main::main(&mut self){'
  write=0
  while [ "$write" -lt 15 ]; do
    printf 'self.console.write_line("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");'
    write=$((write + 1))
  done
  printf 'self.console.write_line("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");self.console.exit_process(0);}'
} > "$T/aggregate-text-exhaust.omg"
run_source "checked aggregate text exhaustion" "$T/aggregate-text-exhaust.omg" 252

cp "$T/case.bundle" "$T/trailing.bundle"; printf x >> "$T/trailing.bundle"
run_bundle "bundle trailing byte" "$T/trailing.bundle" 251

# The reference compiler is only an on-ramp. Compile the frontend once more
# through the Delta-written lowermachine and require the resulting program to
# preserve both an accepted observation and a semantic rejection.
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/lowermachine.alp" "$T/lowermachine" >/dev/null
"$T/lowermachine" < "$FRONTEND" > "$T/frontend-self.s"
clang -arch arm64 -o "$T/frontend-self" "$T/frontend-self.s"
codesign -f -s - "$T/frontend-self" >/dev/null 2>&1
bundle_one "$OMEGA_PATH_CORPUS/cli_mvp/main.omg" "$T/canonical.bundle"
set +e
"$T/frontend-self" < "$T/canonical.bundle" > "$T/self-canonical.terminal" 2>/dev/null; self_ok=$?
"$T/frontend-self" < "$T/auxiliary.bundle" > "$T/self-auxiliary.terminal" 2>/dev/null; self_aux=$?
"$T/frontend-self" < "$T/multi.bundle" > "$T/self-bad.terminal" 2>/dev/null; self_bad=$?
set -e
if [ "$self_ok" = 107 ] && [ "$self_aux" = 107 ] && [ "$self_bad" = 251 ] \
    && cmp -s "$T/self-canonical.terminal" "$T/self-auxiliary.terminal" \
    && [ ! -s "$T/self-bad.terminal" ]; then
  PASS=$((PASS+3))
else
  FAIL=$((FAIL+1)); echo "  FAIL Delta-written self-host path: accepted=$self_ok auxiliary=$self_aux rejected=$self_bad"
fi

echo "omega-bootstrap Delta frontend: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
