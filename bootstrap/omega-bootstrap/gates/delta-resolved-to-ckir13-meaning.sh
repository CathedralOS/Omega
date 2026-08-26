#!/usr/bin/env sh
# Persisted-Beta/Gamma meaning for OMGLOWE -> CKIR13 full-u32 subtraction.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-CKIR13 meaning: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR13 meaning: skipped ($TOOL absent)"
    exit 0
  }
done

C="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER"
G="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES"
LOWERER="$C/omega-bootstrap-resolved-to-ckir4.alp"
RESOLVER="$C/omega-bootstrap-resolve.alp"
FIXTURE="$G/delta-resolved-to-ckir13-meaning-fixture.py"
RUNNER="$G/delta-ckir4-meaning-runner.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
ELABORATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta"
INTERPRETER="$OMEGA_PATH_GAMMA/interp.beta"
for REQUIRED in "$LOWERER" "$RESOLVER" "$FIXTURE" "$RUNNER" \
  "$DECODER" "$ELABORATOR" "$INTERPRETER"; do
  [ -f "$REQUIRED" ] || {
    echo "resolved-to-CKIR13 meaning: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_CKIR13_MEANING_TEMP:-0}" = 1 ]; then
    echo "resolved-to-CKIR13 meaning: retained temporary directory $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"
python3 - "$T/started" <<'PY'
from pathlib import Path
import sys, time
Path(sys.argv[1]).write_text(f"{time.time():.9f}\n", encoding="ascii")
PY

stamp_beta_compiler "$T/bc" >/dev/null
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
build_beta() { # label source executable
  LABEL=$1 SOURCE=$2 EXECUTABLE=$3
  "$T/bc" < "$SOURCE" > "$T/$LABEL.asm" \
    && "$ASM" < "$T/$LABEL.asm" > "$T/$LABEL.tape" \
    && stamp_seed "$T/$LABEL.tape" "$SEED" "$EXECUTABLE" >/dev/null 2>&1
}
build_beta elaborate "$ELABORATOR" "$T/elaborate" & ELABORATE_BUILD=$!
build_beta interpreter "$INTERPRETER" "$T/interpreter" & INTERPRETER_BUILD=$!
set +e
wait "$ELABORATE_BUILD"; ELABORATE_STATUS=$?
wait "$INTERPRETER_BUILD"; INTERPRETER_STATUS=$?
set -e
[ "$ELABORATE_STATUS" -eq 0 ] && [ "$INTERPRETER_STATUS" -eq 0 ] || {
  echo "resolved-to-CKIR13 meaning: persisted-Beta tool build failed (elaborator=$ELABORATE_STATUS interpreter=$INTERPRETER_STATUS)" >&2
  exit 1
}

LOWERER_GAMMA_CEILING=2359296
python3 -B "$RUNNER" capacity-tooth "$LOWERER_GAMMA_CEILING"
python3 -B "$RUNNER" elaborate "$T/elaborate" "$LOWERER" \
  "$T/lowerer.gamma" "$T/timings.tsv" "resolved-to-CKIR13 meaning" \
  40 "$LOWERER_GAMMA_CEILING"

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null & RESOLVER_BUILD=$!
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null & LOWERER_BUILD=$!
set +e
wait "$RESOLVER_BUILD"; RESOLVER_STATUS=$?
wait "$LOWERER_BUILD"; LOWERER_STATUS=$?
set -e
[ "$RESOLVER_STATUS" -eq 0 ] && [ "$LOWERER_STATUS" -eq 0 ] || {
  echo "resolved-to-CKIR13 meaning: native producer build failed (resolver=$RESOLVER_STATUS lowerer=$LOWERER_STATUS)" >&2
  exit 1
}

python3 -B "$FIXTURE" prepare "$T"
"$T/resolver" < "$T/canonical.omgc" > "$T/canonical.omgrsw5"
"$T/resolver" < "$T/underflow.omgc" > "$T/underflow.omgrsw5"
python3 -B "$FIXTURE" frame "$T"
: > "$T/empty.expected"

native_case() { # label expected status
  LABEL=$1 EXPECTED=$2
  set +e
  "$T/lowerer" < "$T/$LABEL.omglowe" > "$T/$LABEL.expected"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "resolved-to-CKIR13 meaning: $LABEL native status $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  [ "$EXPECTED" -eq 0 ] || [ ! -s "$T/$LABEL.expected" ] || {
    echo "resolved-to-CKIR13 meaning: $LABEL native rejection published bytes" >&2
    exit 1
  }
}
native_case canonical 0
native_case underflow 0
native_case semantic-251 251
native_case resource-252 252
python3 -B "$FIXTURE" check "$T"

launch_gamma() { # label
  LABEL=$1
  python3 -B "$RUNNER" run "$T/interpreter" "$T/lowerer.gamma" \
    "$T/$LABEL.omglowe" "$T/$LABEL.observation" "$T/$LABEL.timing" \
    "resolved-to-CKIR13 meaning $LABEL" 180 > "$T/$LABEL.log" 2>&1
}
PIDS=''
for LABEL in canonical underflow semantic-251 resource-252; do
  launch_gamma "$LABEL" &
  PIDS="$PIDS $!:$LABEL"
done
GAMMA_FAILURE=''
set +e
for JOB in $PIDS; do
  PID=${JOB%%:*} LABEL=${JOB#*:}
  wait "$PID"
  ACTUAL=$?
  [ "$ACTUAL" -eq 0 ] || [ -n "$GAMMA_FAILURE" ] || GAMMA_FAILURE="$LABEL:$ACTUAL"
done
set -e
for LABEL in canonical underflow semantic-251 resource-252; do
  cat "$T/$LABEL.log"
  [ -f "$T/$LABEL.timing" ] && cat "$T/$LABEL.timing" >> "$T/timings.tsv"
done
[ -z "$GAMMA_FAILURE" ] || {
  echo "resolved-to-CKIR13 meaning: Gamma observation failed ($GAMMA_FAILURE)" >&2
  exit 1
}

check_gamma() { # label expected status
  LABEL=$1 EXPECTED=$2
  ACTUAL=$(python3 -B "$DECODER" "$T/$LABEL.observation" "$T/$LABEL.stdout")
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "resolved-to-CKIR13 meaning: $LABEL Gamma status $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  cmp "$T/$LABEL.stdout" "$T/$LABEL.expected" >/dev/null || {
    echo "resolved-to-CKIR13 meaning: $LABEL native/Gamma publication differs" >&2
    exit 1
  }
}
check_gamma canonical 0
check_gamma underflow 0
check_gamma semantic-251 251
check_gamma resource-252 252

python3 - "$T/timings.tsv" "$T/started" "$T/canonical.expected" <<'PY'
from pathlib import Path
import sys, time
rows=[]
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds,size,label=line.split("\t",2)
    rows.append((float(seconds),int(size),label))
wall=time.time()-float(Path(sys.argv[2]).read_text(encoding="ascii"))
print("resolved-to-CKIR13 meaning timings: "+" ".join(
    f"{label}={seconds:.2f}s/{size}B" for seconds,size,label in rows
)+f" command-sum={sum(row[0] for row in rows):.2f}s wall={wall:.2f}s")
print("resolved-to-CKIR13 meaning: maximum-minus-near-maximum result 70 and "
      "zero-minus-one runtime trap; semantic=251 resource=252; exact native/Gamma "
      f"publication passed; CKIR13={Path(sys.argv[3]).stat().st_size}B")
PY
