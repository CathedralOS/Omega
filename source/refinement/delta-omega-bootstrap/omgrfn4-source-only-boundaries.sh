#!/usr/bin/env sh
# Focused future-OMGRFN4-R4 source-only evaluator resource boundaries.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN4 source-only boundaries: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN4 source-only boundaries: skipped ($TOOL absent)"
    exit 0
  }
done

EVALUATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-source-only-boundaries.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
LOW_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolution_handoff_reference.py"
for REQUIRED in "$EVALUATOR" "$PACKER" "$RESOLVER" "$LOWERER" "$LOW_FRAME" "$REFERENCE"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN4 source-only boundaries: missing $REQUIRED" >&2; exit 1; }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$EVALUATOR")
[ "$PROCEDURES" -le 128 ] || { echo "OMGRFN4 source-only boundaries: $PROCEDURES procedures" >&2; exit 1; }
MAX_LOCALS=$(python3 - "$EVALUATOR" <<'PY'
import re, sys
source = open(sys.argv[1], encoding="ascii").read()
maximum = 0
for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
    end = source.find("\nproc ", match.end())
    body = source[match.end():end if end >= 0 else len(source)]
    params = sum(bool(item.strip()) for item in match.group(1).split(","))
    maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 source-only boundaries: $MAX_LOCALS locals" >&2; exit 1; }
python3 - "$EVALUATOR" <<'PY'
from pathlib import Path
import re, sys
text = Path(sys.argv[1]).read_text(encoding="ascii")
bad = re.findall(r"(?m)^proc\s+\w*(?:ckir|elf)\w*\s*\(", text, re.I)
if bad:
    raise SystemExit("source-only evaluator physically contains CKIR/ELF reader procedures")
required = (
    "byte[30000000+index]", "36000000+machine*96",
    "36200000+id*80", "word[36300000]",
    "byte[37000000+word[29006000]]", "word[29007008]>=65536", "depth>=16",
)
if any(item not in text for item in required):
    raise SystemExit("source-only boundary/memory-map assertion drifted")
PY

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)

# Run a process in a fresh group. On timeout, terminate the complete group and
# escalate to SIGKILL. The runner also enforces exact status and publication.
timed_run() { # executable input expected output empty-output timeout label
  python3 - "$1" "$2" "$3" "$4" "$5" "$6" "$7" <<'PY'
import os, signal, subprocess, sys, time
exe, input_path, expected, output_path, empty, timeout, label = sys.argv[1:]
started = time.monotonic()
with open(input_path, "rb") as source, open(output_path, "wb") as output:
    proc = subprocess.Popen([exe], stdin=source, stdout=output, stderr=subprocess.PIPE,
                            start_new_session=True)
    try:
        _, stderr = proc.communicate(timeout=float(timeout))
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGTERM)
        try:
            proc.wait(timeout=1.0)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL); proc.wait()
        raise SystemExit(f"OMGRFN4 source-only boundaries: {label} timed out after {timeout}s")
elapsed = time.monotonic() - started
if proc.returncode != int(expected):
    raise SystemExit(f"OMGRFN4 source-only boundaries: {label} returned {proc.returncode}, expected {expected}; stderr={stderr[:400]!r}")
if empty == "yes" and os.path.getsize(output_path):
    raise SystemExit(f"OMGRFN4 source-only boundaries: {label} published output")
print(f"OMGRFN4 source-only boundaries: {label} {elapsed:.3f}s")
PY
}

echo "OMGRFN4 source-only boundaries: building artifact-free persisted-Beta evaluator"
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
"$BC" < "$EVALUATOR" > "$T/evaluator.asm"
"$ASM" < "$T/evaluator.asm" > "$T/evaluator.tape"
TAPE_BYTES=$(wc -c < "$T/evaluator.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 source-only boundaries: tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/evaluator.tape" "$SEED" "$T/evaluator" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null

python3 - "$REFERENCE" "$T" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, str(Path(sys.argv[1]).parent))
from resolution_handoff_reference import one_source
out = Path(sys.argv[2])

def chain(count):
    parts = ["module app;", "data Probe {}"]
    for index in range(count):
        name = "run" if index == 0 else f"m{index}"
        body = "70" if index + 1 == count else f"self.m{index + 1}()"
        parts.append(f"machine Probe::{name}(&mut self) -> u32 {{ {body} }}")
    return "\n".join(parts) + "\n"

def cyclic(bound):
    return f"""module app;
data Probe {{}}
machine Probe::run(&mut self) -> u32 {{
 transition {{ _ -> loop(0) }}
 state loop(&mut self, index: u32 in Trapping) {{
  transition index < {bound} {{
   true -> loop(index + 1)
   _ -> pass()
  }}
 }}
 state pass(&mut self) {{ 70 }}
}}
"""

cases = {
    "frames-16": chain(16), "frames-17": chain(17),
    "entries-65536": cyclic(65533), "entries-65537": cyclic(65534),
}
for name, source in cases.items():
    (out / f"{name}.omg").write_text(source, encoding="ascii")
    (out / f"{name}.omgc").write_bytes(one_source(source, module="app", owner="Probe"))

# entry + loop indices 0..bound + pass = bound + 3.
assert 65533 + 3 == 65536 and 65534 + 3 == 65537
PY

printf opaque-elf > "$T/opaque.elf"
for CASE in frames-16 frames-17 entries-65536 entries-65537; do
  timed_run "$T/resolver" "$T/$CASE.omgc" 0 "$T/$CASE.witness" no 30 "$CASE source->OMGRSW1"
  python3 "$LOW_FRAME" pack "$T/$CASE.omgc" "$T/$CASE.witness" > "$T/$CASE.low"
  timed_run "$T/lowerer" "$T/$CASE.low" 0 "$T/$CASE.ckir" no 30 "$CASE OMGRSW1->CKIR3"
  python3 - "$T/$CASE.ckir" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes()
if raw[:8] != b"OMGCKIR\0" or struct.unpack_from("<H", raw, 8)[0] != 3:
    raise SystemExit("fixture producer did not publish canonical CKIR3")
PY
  python3 "$PACKER" "$T/$CASE.omgc" "$T/$CASE.witness" "$T/$CASE.ckir" \
    "$T/opaque.elf" --result 70 > "$T/$CASE.rfn"
done

timed_run "$T/evaluator" "$T/frames-16.rfn" 0 "$T/frames-16.out" yes 10 "16 active frames/result 70/status 0"
timed_run "$T/evaluator" "$T/frames-17.rfn" 252 "$T/frames-17.out" yes 10 "17th active frame/status 252"
timed_run "$T/evaluator" "$T/entries-65536.rfn" 0 "$T/entries-65536.out" yes 10 "65,536 block entries/result 70/status 0"
timed_run "$T/evaluator" "$T/entries-65537.rfn" 252 "$T/entries-65537.out" yes 10 "65,537th block entry/status 252"

ELAPSED=$(($(date +%s)-STARTED))
echo "OMGRFN4 source-only boundaries: focused future-R4 evaluator caps passed (${ELAPSED}s; ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape bytes; frame 16/17; block entries 65536/65537)"
