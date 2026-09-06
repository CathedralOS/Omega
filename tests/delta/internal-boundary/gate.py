"""Frame selected Gamma inputs and compare exact source-owned outcomes."""

import csv
import hashlib
import struct
import subprocess
import sys
from pathlib import Path


gate = Path(__file__).resolve().parent
temporary = Path(sys.argv[1])
with (gate / "compiler.tsv").open(encoding="ascii", newline="") as stream:
    reader = csv.DictReader(stream, delimiter="\t")
    if reader.fieldnames != ["name", "lines", "bytes", "sha256"]:
        raise SystemExit("Delta internal boundary: unexpected identity header")
    rows = list(reader)
if [row["name"] for row in rows] != ["canonical", "diagnostic"]:
    raise SystemExit("Delta internal boundary: incomplete identity inventory")
sources = {}
for row in rows:
    source = (temporary / (row["name"] + ".gamma")).read_bytes()
    actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
    expected = (int(row["lines"]), int(row["bytes"]), row["sha256"])
    if actual != expected:
        raise SystemExit(f"Delta internal boundary: {row['name']} identity changed: {actual}")
    sources[row["name"]] = source


def frame(tag, space, code, coordinate, limit=0, requested=0):
    return b"\xffDCOUT\x01\x00" + struct.pack(
        "<BBHIQQQ", tag, space, 0, code, coordinate, limit, requested
    )


def observe(name, subject, sealed_input, status, output):
    source = sources[subject]
    try:
        process = subprocess.run(
            [str(temporary / "evaluator.exe")],
            input=struct.pack("<I", len(source)) + source + sealed_input,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"Delta internal boundary {name}: timed out after 30 seconds")
    if (process.returncode, process.stdout, process.stderr) != (status, output, b""):
        raise SystemExit(
            f"Delta internal boundary {name}: expected {status}/{output.hex()}, "
            f"got {process.returncode}/{process.stdout.hex()}, stderr={process.stderr!r}"
        )


typing_controls = (
    ("unknown_above_range", 0, 3, frame(3, 3, 1, 0)),
    ("unknown_zero_at_row_two", 1, 3, frame(3, 3, 1, 2)),
    ("unknown_negative_at_row_one", 2, 3, frame(3, 3, 1, 1)),
    ("empty_stack", 3, 0, b"\x01"),
    ("valid_false_branch", 4, 0, b"\x01"),
    ("nonpair_frame", 5, 249, b""),
    ("nonpair_retained_fields", 6, 249, b""),
    ("negative_depth", 7, 249, b""),
)
metadata_controls = (
    ("negative_body_profile_one", 8, 3, frame(3, 3, 2, 0)),
    ("negative_definition_name", 9, 3, frame(3, 3, 2, 0)),
    ("negative_parameter_name", 10, 3, frame(3, 3, 2, 0)),
    ("negative_call_argument_before_positive_sibling", 11, 3, frame(3, 3, 2, 0)),
    ("negative_nested_call_in_let_body", 12, 3, frame(3, 3, 2, 0)),
    ("negative_written", 13, 0, b"\x01"),
    ("minimum_written_normalizes", 14, 0, b"\x01"),
    ("negative_amount", 15, 0, b"\x01"),
    ("minimum_amount_normalizes", 16, 0, b"\x01"),
    ("negative_written_before_maximum_amount", 17, 0, b"\x01"),
    ("zero_count", 18, 0, b"\x01"),
    ("maximum_count_unchanged", 19, 0, b"\x01"),
    ("exact_maximum_sum", 20, 0, b"\x01"),
    ("positive_overflow_by_one", 21, 249, b""),
    ("positive_overflow_from_adjacent_operands", 22, 249, b""),
    ("valid_publication", 23, 0, b"(def f () Int 0)\n\n"),
    # 14-byte definition prefix + cached body + ')' + LF + final entry LF.
    ("positive_payload_extent", 24, 2, frame(2, 2, 12, 16777212, 16777212, 16777229)),
)
for name, mode, status, output in typing_controls + metadata_controls:
    for repetition in range(2):
        observe(f"{name}/{repetition + 1}", "diagnostic", bytes([mode]), status, output)

# Authored source companions use the complete canonical admission pipeline.
# The offsets are explicit expectations, not derived by a host syntax model.
for name, source, code, coordinate in (
    ("unexpected_close", b")", 4, 0),
    ("unknown_local", b"(def main ((source Bytes)) Bytes missing)", 14, 33),
):
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source)) + source
    observe(name, "canonical", request, 1, frame(1, 1, code, coordinate))

print("Delta internal boundary: 8 typing and 17 emission controls twice, plus 2 authored rejections passed (52 observations)")
