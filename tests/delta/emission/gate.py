"""Frame selected-evaluator inputs and compare source-owned observations."""

import csv
import hashlib
import struct
import subprocess
import sys
from pathlib import Path

from fixtures import CASES


gate = Path(__file__).resolve().parent
temporary = Path(sys.argv[1])
compiler = (temporary / "compiler.gamma").read_bytes()
with (gate / "compiler.tsv").open(encoding="ascii", newline="") as stream:
    inventory = csv.DictReader(stream, delimiter="\t")
    if inventory.fieldnames != ["lines", "bytes", "sha256"]:
        raise SystemExit("Delta emission: unexpected compiler identity header")
    rows = list(inventory)
actual = (len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest())
if len(rows) != 1:
    raise SystemExit(f"Delta emission: diagnostic identity not registered; measured {actual}")
expected_identity = (int(rows[0]["lines"]), int(rows[0]["bytes"]), rows[0]["sha256"])
if actual != expected_identity:
    raise SystemExit(f"Delta emission: compiler identity changed: {actual}")


def observe(name, sealed_input, expected):
    try:
        process = subprocess.run(
            [str(temporary / "evaluator")],
            input=struct.pack("<I", len(compiler)) + compiler + sealed_input,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"Delta emission {name}: selected Gamma timed out after 30 seconds")
    if process.returncode != 0 or process.stderr or process.stdout != expected:
        raise SystemExit(
            f"Delta emission {name}: expected status 0 / {expected.hex()}, "
            f"got {process.returncode} / {process.stdout.hex()}, stderr={process.stderr!r}"
        )


for name, mode, source, published, extent, cache in CASES:
    if len(published) != extent:
        raise SystemExit(f"Delta emission {name}: authored expectation length contradicts extent")
    expected = (
        struct.pack("<II", extent, 11 + extent) + bytes([cache])
        + published + struct.pack("<I", 11 + extent) + b"\x00"
    )
    for repetition in range(2):
        observe(f"{name}/{repetition + 1}", bytes([mode]) + source, expected)

# No negative packed bytes are published: the source-owned classifier alone
# reports zero fallback metadata, followed by unmarked Gamma main's final zero.
for repetition in range(2):
    observe(f"negative_word_metadata/{repetition + 1}", b"\x12", b"\x00" * 5)
print("Delta emission: 19 exact serializer controls passed twice (38 observations)")
