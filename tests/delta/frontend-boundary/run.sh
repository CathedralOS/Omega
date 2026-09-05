#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta frontend boundary: skipped (python3 absent)"
    exit 0
}

FRONTEND_BOUNDARY_TMP=$(mktemp -d)
trap 'rm -rf -- "$FRONTEND_BOUNDARY_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$FRONTEND_BOUNDARY_TMP/compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
materialize_gamma_evaluator "$FRONTEND_BOUNDARY_TMP/evaluator" >/dev/null

FRONTEND_BOUNDARY_TMP="$FRONTEND_BOUNDARY_TMP" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

directory = Path(os.environ["FRONTEND_BOUNDARY_TMP"])
compiler = (directory / "compiler.gamma").read_bytes()
identity = (len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest())
if identity != (
    2326, 96148, "bab9afe19dec17995fc0b50355bb0b90033195b4a35c929a9f19a939dcd55162"
):
    raise SystemExit(f"Delta compiler identity changed: {identity}")

REQUEST_MAGIC = b"DCREQ\x01\x00\x00"
OUTCOME_MAGIC = b"\xffDCOUT\x01\x00"
identity_source = b"(def main ((source Bytes)) Bytes source)\n"


def evaluate(program, sealed_input):
    process = subprocess.Popen(
        [str(directory / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input, timeout=30
        )
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Delta frontend boundary: selected Gamma timed out")
    if error:
        raise SystemExit(f"unexpected evaluator stderr: {error!r}")
    return process.returncode, output


def request(source):
    return REQUEST_MAGIC + struct.pack("<II", 1, len(source)) + source


def rejection(code, coordinate, space=1):
    frame = struct.pack(
        "<8sBBHIQQQ", OUTCOME_MAGIC, 1, space, 0, code, coordinate, 0, 0
    )
    assert len(frame) == 40
    return 1, frame


cases = []
for byte in (0, 1, 8, 11, 12, 14, 31, 127, 128, 255):
    cases.append((f"invalid source byte {byte}", bytes([byte]) + identity_source,
                  rejection(3, 0)))
cases.extend((
    ("Unicode BOM is source bytes", b"\xef\xbb\xbf" + identity_source, rejection(3, 0)),
    ("invalid byte inside comment", b"; text\x00\n" + identity_source, rejection(3, 6)),
    ("source byte before syntax", b"(\x00", rejection(3, 1)),
    ("first invalid source byte", b" \x00\xff" + identity_source, rejection(3, 1)),
    ("source byte before duplicate", b"\x00(def f () Int 0)\n(def f () Int 1)\n",
     rejection(3, 0)),
    ("later source byte before duplicate collection",
     b"(def f () Int 0)\n(def f () Int 1)\n\x00", rejection(3, 34)),
))

# Literal authored coordinates are fixture expectations. The host does not
# tokenize source, locate declarations, or select a diagnostic candidate.
for name, source, code, coordinate in (
    ("duplicate type", b"(data Item (First))\n(data Item (Second))\n(def main ((source Bytes)) Bytes source)\n", 6, 26),
    ("duplicate constructor across owners", b"(data First (Item))\n(data Second (Item))\n(def main ((source Bytes)) Bytes source)\n", 7, 34),
    ("duplicate constructor in owner", b"(data First (Item) (Item))\n(def main ((source Bytes)) Bytes source)\n", 7, 20),
    ("duplicate function", b"(def helper () Int 1)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 27),
    ("duplicate main", b"(def main ((source Bytes)) Bytes source)\n(def main () Int 1)\n", 8, 46),
    ("unknown constructor type before duplicate type", b"(data First (One Unknown))\n(data First (Two))\n(def main ((source Bytes)) Bytes source)\n", 6, 33),
    ("unknown constructor type before duplicate constructor", b"(data First (One Unknown))\n(data Second (One))\n(def main ((source Bytes)) Bytes source)\n", 7, 41),
    ("unknown signature type before duplicate function", b"(def helper ((value Unknown)) Int 1)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 42),
    ("duplicate before unknown constructor type", b"(data First (One))\n(data First (Two Unknown))\n(def main ((source Bytes)) Bytes source)\n", 6, 25),
    ("duplicate before unknown signature type", b"(def helper () Int 1)\n(def helper ((value Unknown)) Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 27),
    ("body error before duplicate function", b"(def helper () Int missing)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 33),
    ("earliest duplicate across namespaces", b"(data Item (One))\n(data Item (Two))\n(def helper () Int 1)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 6, 24),
    ("earlier constructor duplicate before later type duplicate", b"(data First (One))\n(data Second (One))\n(data First (Two))\n(def main ((source Bytes)) Bytes source)\n", 7, 33),
    ("wrong entry arity", b"(def main () Int 7)\n", 20, 5),
    ("wrong entry parameter", b"(def main ((source Int)) Bytes (bytes_empty))\n", 20, 5),
    ("wrong entry result", b"(def main ((source Bytes)) Int 7)\n", 20, 5),
    ("wrong entry two parameters", b"(def main ((left Bytes) (right Bytes)) Bytes left)\n", 20, 5),
    ("wrong entry name after declarations and comments",
     b"; prefix\n(data Item (Item))\n(def helper () Int 0)\n"
     b"(def ; entry\n  main () Int 7)\n", 20, 65),
):
    cases.append((name, source, rejection(code, coordinate)))

cases.append(("valid frontend without main", b"(def helper () Int 0)\n",
              rejection(19, 0, space=0)))
cases.append(("entry prefix is not main", b"(def mai () Int 0)\n",
              rejection(19, 0, space=0)))
cases.append(("entry suffix is not main", b"(def main_suffix () Int 0)\n",
              rejection(19, 0, space=0)))

# These unfinished frontend paths must remain evaluator-owned failures, not
# guessed DCOUT frames or schema diagnostics derived before frontend success.
for name, source in (
    ("empty source", b""),
    ("invalid syntax", b"("),
    ("unknown signature type without main", b"(def helper ((value Unknown)) Int 0)\n"),
    ("unknown constructor type without main", b"(data Item (Item Unknown))\n(def helper () Int 0)\n"),
    ("unknown body name without main", b"(def helper () Int missing)\n"),
    ("wrong present entry with body error", b"(def main () Int missing)\n"),
    ("wrong present entry with body type error", b"(def main () Int (bytes_empty))\n"),
    ("ordinary body error before valid entry", b"(def helper () Int missing)\n" + identity_source),
):
    cases.append((name, source, (249, b"")))

for name, source, expected in cases:
    actual = evaluate(compiler, request(source))
    if actual != expected:
        raise SystemExit(
            f"{name}: expected status {expected[0]} and {expected[1].hex()}, "
            f"got status {actual[0]}, {len(actual[1])} bytes, prefix {actual[1][:80].hex()}"
        )

accepted = (
    ("identity", identity_source),
    ("exact entry after longer function name",
     b"(def main_suffix () Int 0)\n" + identity_source),
    ("grammar-distinguished namespace reuse",
     b"(data Token (Token Int))\n"
     b"(def f ((f Bytes)) Bytes f)\n"
     b"(def main ((source Bytes)) Bytes (f source))\n"),
    ("forward and mutually visible data",
     b"(data Left (End) (ToRight Right))\n"
     b"(data Right (ToLeft Left))\n" + identity_source),
    ("forward and mutually visible functions",
     b"(def first ((value Int)) Int (if value (second 0) 0))\n"
     b"(def second ((value Int)) Int (if value (first 0) 0))\n"
     b"(def main ((source Bytes)) Bytes (if (first 1) (bytes_empty) source))\n"),
    ("textual ASCII whitespace and comment endings",
     b"; comment\r\t" + identity_source + b"; final comment"),
)
payload = b"\x00A\x80\xff"
for name, source in accepted:
    status, receipt = evaluate(compiler, request(source))
    if status != 0 or not receipt:
        raise SystemExit(f"{name}: expected nonempty successful receipt, got {status}, {receipt[:80].hex()}")
    if evaluate(compiler, request(source)) != (0, receipt):
        raise SystemExit(f"{name}: compilation did not reconstruct identical bytes")
    if evaluate(receipt, payload) != (0, payload):
        raise SystemExit(f"{name}: compiled application did not preserve exact binary input")

frames = sum(len(expected[1]) == 40 for _, _, expected in cases)
print(
    f"Delta frontend boundary: {frames} exact DCOUT controls, "
    f"{len(cases) - frames} evaluator-owned failures, "
    f"{len(accepted)} repeated accepted compilations and application observations passed"
)
PY
