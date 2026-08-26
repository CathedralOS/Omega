#!/usr/bin/env sh
# Raw-unit source-custody probe. The fast native checker carries the exhaustive
# semantic/resource matrix; a representative matrix is repeated through the
# much slower Delta-written lowermachine producer.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "source-custody frontend: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "source-custody frontend: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "source-custody frontend: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
ACTUAL="$OMEGA_REPO_ROOT/source/compiler/omega/psi/source/source.omg"
[ -f "$CHECKER" ] || { echo "source-custody frontend: checker source absent" >&2; exit 1; }
[ -f "$ACTUAL" ] || { echo "source-custody frontend: product source fixture absent" >&2; exit 1; }

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$CHECKER" "$T/checker.native" >/dev/null
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/lowermachine" >/dev/null
if ! "$T/lowermachine" < "$CHECKER" > "$T/checker.self.s"; then
  echo "source-custody frontend: lowermachine could not compile checker" >&2
  exit 1
fi
clang -arch arm64 -o "$T/checker.self" "$T/checker.self.s"
codesign -f -s - "$T/checker.self" >/dev/null 2>&1

mkdir "$T/cases"
python3 - "$T/cases" "$ACTUAL" <<'PY'
from pathlib import Path
import sys

out = Path(sys.argv[1])
actual = Path(sys.argv[2]).read_bytes()
rows = []

def case(name, expected, source):
    path = out / (name + ".omg")
    if isinstance(source, str):
        source = source.encode("ascii")
    path.write_bytes(source)
    rows.append((expected, name, path))

case("actual-source-unit", 0, actual)
case("renamed-reordered-unit", 0, r'''
data LocatedBytes [copy] { region: Region; file: FileKey; }
data Region [copy] { finish: u32 in Trapping; begin: u32 in Trapping; }
data FileKey [copy] { raw: u32 in Trapping; }
data ByteStore {
    retained: bool;
    length: u32 [0..=65536];
    file: FileKey;
    bytes: [u8; 65536] in Trapping;
}
machine ByteStore::byte_or_zero(&self, at: u32 in Trapping) -> u8 {
    transition at < self.length { true -> present() _ -> absent() }
    state absent(&self) { 0 }
    state present(&self) { self.bytes[at] }
}
machine ByteStore::append(&mut self, value: u8) {
    self.retained = false;
    transition self.length < 65536 { true -> keep() _ -> full() }
    state full(&mut self) { self.retained = false; }
    state keep(&mut self) {
        self.bytes[self.length] = value;
        self.length = self.length + 1;
        self.retained = true;
    }
}
machine ByteStore::clear(&mut self, file: FileKey) {
    self.file = file;
    self.length = 0;
    self.retained = true;
}
''')

# Phase-isolated malformed/unsupported/typing controls.
case("reject-syntax", 251, "data Broken { value: u8;")
case("reject-duplicate-root", 251, "data Same { x: u8; } data Same { y: u8; }")
case("reject-duplicate-field", 251, "data Pair { x: u8; x: u8; }")
case("reject-unknown-type", 251, "data Holder { value: Missing; }")
case("reject-shared-mutation", 251, r'''data Cell { value: u8; }
machine Cell::bad(&self) { self.value = 1; }''')
case("reject-noncopy-assignment", 251, r'''data Inner { value: u8; }
data Outer { left: Inner; right: Inner; }
machine Outer::bad(&mut self) { self.left = self.right; }''')
case("reject-type-mismatch", 251, r'''data Cell { flag: bool; }
machine Cell::bad(&mut self, value: u8) { self.flag = value; }''')
case("reject-missing-target", 251, r'''data Cell { flag: bool; }
machine Cell::bad(&mut self) { transition self.flag { true -> missing() _ -> done() } state done(&mut self) { } }''')
case("reject-target-arity", 251, r'''data Cell { flag: bool; }
machine Cell::bad(&mut self) { transition self.flag { true -> done() _ -> done() } state done(&mut self, value: u8) { } }''')
case("reject-nonboolean-guard", 251, r'''data Cell { value: u8; }
machine Cell::bad(&mut self) { transition self.value { true -> done() _ -> done() } state done(&mut self) { } }''')
case("reject-result-mismatch", 251, r'''data Cell { value: u8; }
machine Cell::bad(&self) -> u8 { true }''')
case("reject-bool-trapping", 251, "data Cell { value: bool in Trapping; }")
case("reject-nominal-trapping", 251, r'''data Inner { value: u8; }
data Outer { value: Inner in Trapping; }''')
case("reject-unguarded-index", 251, r'''data Buffer { bytes: [u8; 8] in Trapping; length: u32 [0..=8]; }
machine Buffer::bad(&self, at: u32 in Trapping) -> u8 { self.bytes[at] }''')

# Each public ceiling is exercised at the admitted value and one beyond it.
def padded(size):
    if len(actual) > size:
        raise SystemExit("actual source unexpectedly exceeds source-byte tooth")
    return actual + b" " * (size - len(actual))

case("limit-source-bytes-131072", 0, padded(131072))
case("limit-source-bytes-131073", 252, padded(131073))

def roots(count):
    return "\n".join(f"data Root{i:03d} {{ value: u8; }}" for i in range(count))
case("limit-root-items-128", 0, roots(128))
case("limit-root-items-129", 252, roots(129))

def fields(count):
    return "data Fields { " + " ".join(f"f{i}: u8;" for i in range(count)) + " }"
case("limit-fields-64", 0, fields(64))
case("limit-fields-65", 252, fields(65))

def states(count):
    declarations = " ".join(f"state s{i:03d}(&mut self) {{ }}" for i in range(count))
    return ("data StateHost { flag: bool; }\n"
            "machine StateHost::walk(&mut self) { "
            "transition self.flag { true -> s000() _ -> s000() } "
            + declarations + " }")
# The implicit machine entry is one state in the compiler census.
case("limit-states-128", 0, states(127))
case("limit-states-129", 252, states(128))

def params(count):
    arguments = ", ".join(f"p{i}: u8" for i in range(count))
    return f"data ParamHost {{ value: u8; }} machine ParamHost::take(&mut self, {arguments}) {{ self.value = p0; }}"
# The receiver/self parameter is one parameter in the compiler census.
case("limit-parameters-8", 0, params(7))
case("limit-parameters-9", 252, params(8))

def statements(count):
    body = " ".join(f"self.value = {i};" for i in range(count))
    return f"data StatementHost {{ value: u8; }} machine StatementHost::run(&mut self) {{ {body} }}"
case("limit-statements-32", 0, statements(32))
case("limit-statements-33", 252, statements(33))

case("limit-identifier-bytes-64", 0, "data " + "A" * 64 + " { value: u8; }")
case("limit-identifier-bytes-65", 252, "data " + "A" * 65 + " { value: u8; }")

def expression_depth(depth):
    expression = "1"
    for _ in range(depth - 1):
        expression = "1 + (" + expression + ")"
    return ("data ExprHost { value: u32 in Trapping; } "
            f"machine ExprHost::set(&mut self) {{ self.value = {expression}; }}")
case("limit-expression-depth-8", 0, expression_depth(8))
case("limit-expression-depth-9", 252, expression_depth(9))

def member_path(components):
    data = []
    for i in range(components):
        data.append(f"data Path{i} {{ next: Path{i + 1}; }}")
    data.append(f"data Path{components} {{ value: u8; }}")
    path = "self" + ".next" * components + ".value"
    return "\n".join(data) + f"\nmachine Path0::read(&self) -> u8 {{ {path} }}"
# Member suffixes are AST nodes, not checkpoint `path.components`. Seven
# suffixes over the primary have normalized expression depth 8; eight have 9.
case("limit-member-expression-depth-8", 0, member_path(6))
case("limit-member-expression-depth-9", 252, member_path(7))

case("limit-array-length-65536", 0, "data ArrayHost { bytes: [u8; 65536] in Trapping; }")
case("limit-array-length-65537", 252, "data ArrayHost { bytes: [u8; 65537] in Trapping; }")

with (out / "manifest.tsv").open("w", encoding="utf-8") as manifest:
    for expected, name, path in rows:
        manifest.write(f"{expected}\t{name}\t{path}\n")
PY

python3 - "$T/cases/manifest.tsv" "$T/checker.native" "$T/checker.self" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

manifest = Path(sys.argv[1])
native = sys.argv[2]
self_built = sys.argv[3]
rows = []
for row in manifest.read_text(encoding="utf-8").splitlines():
    expected_text, label, input_text = row.split("\t")
    rows.append((int(expected_text), label, Path(input_text)))

def observe(executable, input_path, timeout):
    started = time.monotonic()
    try:
        with input_path.open("rb") as source:
            result = subprocess.run(
                [executable], stdin=source, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, timeout=timeout, check=False,
            )
    except subprocess.TimeoutExpired:
        return None, None, time.monotonic() - started
    return result.returncode, len(result.stdout), time.monotonic() - started

native_failed = 0
native_elapsed = 0.0
for expected, label, input_path in rows:
    status, byte_count, elapsed = observe(native, input_path, 10)
    native_elapsed += elapsed
    if status is None:
        native_failed += 1
        print(f"  FAIL {label}: native checker exceeded 10 seconds", file=sys.stderr)
    elif status != expected or byte_count != 0:
        native_failed += 1
        print(
            f"  FAIL {label}: native={status}/{byte_count}B "
            f"expected={expected}/0B",
            file=sys.stderr,
        )

if native_failed:
    print(
        f"source-custody frontend: {native_failed} of {len(rows)} native cases failed",
        file=sys.stderr,
    )
    raise SystemExit(1)

# The self-built checker executes the same program. Repeat source custody,
# name/order independence, two distinct semantic failures, one accepted exact
# resource boundary, and one exhausted adjacent boundary. Exhaustive resource
# isolation remains native so this gate cannot degrade into 34 serial
# compiler-sized reference executions.
self_labels = {
    "actual-source-unit",
    "renamed-reordered-unit",
    "reject-shared-mutation",
    "reject-unguarded-index",
    "limit-root-items-128",
    "limit-array-length-65537",
}
self_rows = [row for row in rows if row[1] in self_labels]
if len(self_rows) != len(self_labels):
    raise SystemExit("source-custody frontend: representative self matrix incomplete")

max_self_elapsed = 0.0
self_elapsed = 0.0
for expected, label, input_path in self_rows:
    status, byte_count, elapsed = observe(self_built, input_path, 30)
    self_elapsed += elapsed
    max_self_elapsed = max(max_self_elapsed, elapsed)
    if status is None:
        print(f"  FAIL {label}: self-built checker exceeded 30 seconds", file=sys.stderr)
        raise SystemExit(1)
    if status != expected or byte_count != 0:
        print(
            f"  FAIL {label}: self={status}/{byte_count}B "
            f"expected={expected}/0B",
            file=sys.stderr,
        )
        raise SystemExit(1)

print(
    f"source-custody frontend: {len(rows)} native cases in "
    f"{native_elapsed:.2f}s; {len(self_rows)} representative lowermachine-built "
    f"cases in {self_elapsed:.2f}s (slowest {max_self_elapsed:.2f}s)"
)
PY
