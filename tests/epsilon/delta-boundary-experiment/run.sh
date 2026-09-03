#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta boundary experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
EPSILON="$OMEGA_REPO_ROOT/source/epsilon/compiler/epsilon_compiler.delta"
DELTA="$OMEGA_REPO_ROOT/source/delta/compiler/delta_compiler.gamma"
ANALYSIS=$(python3 "$GATE_DIR/analyze.py" "$EPSILON")

expect_metric() {
    printf '%s\n' "$ANALYSIS" | grep -Fx "$1" >/dev/null || {
        printf '%s\n' "$ANALYSIS"
        echo "Delta boundary metric changed: $1" >&2
        exit 1
    }
}

expect_metric 'source_lines=8733'
expect_metric 'top_level_forms=663'
expect_metric 'data_forms=158'
expect_metric 'definition_forms=505'
expect_metric 'optional_forms=7'
expect_metric 'optional_lines=21'
expect_metric 'optional_shapes=0/1:3,0/3:4'
expect_metric 'parse_outcome_forms=25'
expect_metric 'parse_outcome_lines=75'
expect_metric 'parse_outcome_shapes=1/2:18,2/2:7'
expect_metric 'recursive_list_forms=26'
expect_metric 'recursive_list_lines=84'
expect_metric 'recursive_list_shapes=0/2:25,0/3:1'
expect_metric 'reverse_function_forms=23'
expect_metric 'reverse_function_lines=167'
expect_metric 'list_count_function_forms=3'
expect_metric 'list_count_function_lines=14'
expect_metric 'catalog_lookup_forms=7'
expect_metric 'catalog_lookup_lines=23'
expect_metric 'catalog_lookup_shapes=0/1:4,0/1/2:1,0/2:1,0/2/2:1'
expect_metric 'catalog_function_forms=9'
expect_metric 'catalog_function_lines=183'
expect_metric 'span_function_forms=29'
expect_metric 'span_function_lines=164'
expect_metric 'candidate_forms=3'
expect_metric 'candidate_lines=10'
expect_metric 'minimum_function_forms=6'
expect_metric 'minimum_function_lines=67'
expect_metric 'generic_sum_gross_ceiling_lines=96'
expect_metric 'generic_list_gross_ceiling_lines=265'
expect_metric 'catalog_gross_ceiling_lines=206'
expect_metric 'span_gross_ceiling_lines=164'
expect_metric 'candidate_gross_ceiling_lines=77'
expect_metric 'combined_gross_ceiling_lines=808'
expect_metric 'combined_gross_ceiling_per_mille=92'

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EVALUATOR="$TMP/evaluator" DELTA="$DELTA" GATE_DIR="$GATE_DIR" python3 - <<'PY'
import hashlib
import os
import struct
import subprocess
from pathlib import Path


def evaluate(program, sealed_input=b""):
    request = struct.pack("<I", len(program)) + program + sealed_input
    process = subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )
    return process.returncode, process.stdout


compiler = Path(os.environ["DELTA"]).read_bytes()
root = Path(os.environ["GATE_DIR"])
artifacts = {
    "span_flat": (15, 597, "5c448968a71639645e1472ba716546c219b768fca5803ea6f7580623d928bf5c", 1071,
                  "537ad0e29bc17b69f8f878c170ce38304e8fa7ccccc071a6baff1b7996603848", b"\x0b"),
    "span_wrapped": (19, 759, "8827d8f749a8f5af7609006d461006a51684c3871fb32bbf76878e2ae471d83b", 976,
                     "0a7d974c6de6584c0f41eef4ce82c6ebad45172f2a122830ce1ecd1ba6355f9d", b"\x0b"),
    "candidate_specialized": (54, 2533, "e379dc61da4f4a21d3ba95aed7c83a7fcf9d071b8cd6553722d7564145ba58ef", 3290,
                              "2b14c45500d262c8abb03fff7bcbb8d1ac3e3faa448fd45914d917da4c0cb23a", b"!"),
    "candidate_shared": (59, 2770, "4ea61a46da72f227290529fea23f3383e6b20af7d51610891bfd27be1df59ccf", 3487,
                         "88ae471c1b9d43c61e81746141b6944141ec2f5b62f32a6fdd8a3c36330411c3", b"!"),
}
for name, (lines, size, digest, receipt_size, receipt_digest, result) in artifacts.items():
    source = root.joinpath(f"{name}.delta").read_bytes()
    if len(source.splitlines()) != lines or len(source) != size:
        raise SystemExit(f"{name} source size changed")
    if hashlib.sha256(source).hexdigest() != digest:
        raise SystemExit(f"{name} source identity changed")
    status, receipt = evaluate(compiler, source)
    if status != 0 or len(receipt) != receipt_size:
        raise SystemExit(f"{name} did not lower")
    if hashlib.sha256(receipt).hexdigest() != receipt_digest:
        raise SystemExit(f"{name} receipt identity changed")
    if evaluate(receipt) != (0, result):
        raise SystemExit(f"{name} result changed")

proposals = {
    "generic_option": "a258646c3953b21555f205738ce9b9fa94204a67d37ec8236aad29d7ecf54791",
    "generic_list": "7f30956ace3be7d87d23d72d7aa90249c3fb80e2f86ae82d77fd64b9d91c0ca3",
    "generic_map": "585f7d7c935341de1d563bd2f1b7fc485493218fd643b1c39d3756e833f40206",
}
for name, digest in proposals.items():
    source = root.joinpath(f"{name}.delta-plus").read_bytes()
    if hashlib.sha256(source).hexdigest() != digest:
        raise SystemExit(f"{name} proposal identity changed")
    if evaluate(compiler, source) != (2, b""):
        raise SystemExit(f"{name} unexpectedly entered current Delta")
PY

echo "Delta boundary experiment: generic list remains plausible; sums, map, span wrapper, and candidate fold do not earn Delta expansion"
