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
EPSILON="$TMP/epsilon_compiler.delta"
DELTA="$TMP/delta_compiler.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$DELTA" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_DEVELOPMENT_ENTRY"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON"
ANALYSIS=$(python3 "$GATE_DIR/analyze.py" "$EPSILON")

expect_metric() {
    printf '%s\n' "$ANALYSIS" | grep -Fx "$1" >/dev/null || {
        printf '%s\n' "$ANALYSIS"
        echo "Delta boundary metric changed: $1" >&2
        exit 1
    }
}

expect_metric 'source_lines=11763'
expect_metric 'top_level_forms=881'
expect_metric 'data_forms=181'
expect_metric 'definition_forms=700'
expect_metric 'optional_forms=8'
expect_metric 'optional_lines=24'
expect_metric 'optional_shapes=0/1:4,0/3:4'
expect_metric 'parse_outcome_forms=25'
expect_metric 'parse_outcome_lines=75'
expect_metric 'parse_outcome_shapes=1/2:18,2/2:7'
expect_metric 'recursive_list_forms=30'
expect_metric 'recursive_list_lines=98'
expect_metric 'recursive_list_shapes=0/2:27,0/3:3'
expect_metric 'ordinary_list_forms=27'
expect_metric 'ordinary_list_lines=87'
expect_metric 'reverse_function_forms=23'
expect_metric 'reverse_function_lines=167'
expect_metric 'template_reverse_function_forms=22'
expect_metric 'template_reverse_function_lines=153'
expect_metric 'list_count_function_forms=3'
expect_metric 'list_count_function_lines=14'
expect_metric 'catalog_lookup_forms=8'
expect_metric 'catalog_lookup_lines=26'
expect_metric 'catalog_lookup_shapes=0/1:4,0/1/2:1,0/2:2,0/2/2:1'
expect_metric 'catalog_function_forms=11'
expect_metric 'catalog_function_lines=219'
expect_metric 'span_function_forms=30'
expect_metric 'span_function_lines=168'
expect_metric 'candidate_forms=3'
expect_metric 'candidate_lines=10'
expect_metric 'minimum_function_forms=6'
expect_metric 'minimum_function_lines=67'
expect_metric 'generic_sum_gross_ceiling_lines=99'
expect_metric 'generic_list_gross_ceiling_lines=279'
expect_metric 'exact_list_family_forms=52'
expect_metric 'exact_list_family_lines=254'
expect_metric 'exact_list_family_bytes=11809'
expect_metric 'catalog_gross_ceiling_lines=245'
expect_metric 'span_gross_ceiling_lines=168'
expect_metric 'candidate_gross_ceiling_lines=77'
expect_metric 'combined_gross_ceiling_lines=868'
expect_metric 'combined_gross_ceiling_per_mille=73'

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EVALUATOR="$TMP/evaluator" DELTA="$DELTA" EPSILON="$EPSILON" \
    GATE_DIR="$GATE_DIR" python3 - <<'PY'
import hashlib
import os
import runpy
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
    "span_flat": (15, 597, "5c448968a71639645e1472ba716546c219b768fca5803ea6f7580623d928bf5c", 1235,
                  "65a0e610dc6594f191b59a2a8f574f7a5ac5dce719adbe066028ccc6f842f612", b"\x0b"),
    "span_wrapped": (19, 759, "8827d8f749a8f5af7609006d461006a51684c3871fb32bbf76878e2ae471d83b", 1153,
                     "136e5b409333917dff29d7e54b396de8d270d8f39e0f3db2e280abd79f562e9e", b"\x0b"),
    "candidate_specialized": (54, 2533, "e379dc61da4f4a21d3ba95aed7c83a7fcf9d071b8cd6553722d7564145ba58ef", 3837,
                              "9b2cc80d8ec4e0d59f5678c526467033574a2de61883c637bdf907a759f9344a", b"!"),
    "candidate_shared": (59, 2770, "4ea61a46da72f227290529fea23f3383e6b20af7d51610891bfd27be1df59ccf", 4050,
                         "8c1837e9e5ea72c7eccb110f262a96b0853fd3cbe233cd236cada3e4cf4fd44d", b"!"),
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

list_artifacts = {
    "list_elaborator.gamma": (292, 13200, "bda48281d6a61cb4e6fd76f40e7cecc0f9d65c72660114dcfc0363009b884f78"),
    "list_family.delta-plus": (27, 3795, "1c4f623f1c170141c428c458d5d45f116658cfb4cc0566eb20fc8053232eb76f"),
    "list_smoke.delta-plus": (3, 194, "26ead8ae1fb420f914910065c0e51b4e505a0864bd8f11b2e02d323ee4d2b685"),
    "list_smoke.delta": (6, 464, "4e3a8ba3f4bc04a3a3a11d63efb5ba0f848519c0ec6e5c3b37ec2634b0edcd6b"),
}
for name, (lines, size, digest) in list_artifacts.items():
    source = root.joinpath(name).read_bytes()
    if len(source.splitlines()) != lines or len(source) != size:
        raise SystemExit(f"{name} size changed")
    if hashlib.sha256(source).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

elaborator = root.joinpath("list_elaborator.gamma").read_bytes()
smoke = root.joinpath("list_smoke.delta-plus").read_bytes()
expected_smoke = root.joinpath("list_smoke.delta").read_bytes()
if evaluate(elaborator, smoke) != (0, expected_smoke):
    raise SystemExit("derived-list smoke expansion changed")
status, smoke_receipt = evaluate(compiler, expected_smoke)
if status != 0 or evaluate(smoke_receipt) != (0, b"\x02"):
    raise SystemExit("derived-list smoke did not compose through selected Delta")

family_spec = root.joinpath("list_family.delta-plus").read_bytes()
status, family_expansion = evaluate(elaborator, family_spec)
if status != 0 or len(family_expansion) != 11102:
    raise SystemExit("derived Epsilon list family did not expand")
if hashlib.sha256(family_expansion).hexdigest() != "76625698f4693ebc5989c6948daa9c1ae4586902e26bee3c6e2aaf28ede1cc40":
    raise SystemExit("derived Epsilon list family identity changed")

analyzer = runpy.run_path(str(root / "analyze.py"), run_name="boundary_analyzer")
parse_forms = analyzer["parse_forms"]
is_def = analyzer["is_def"]
recursive_list = analyzer["recursive_list"]
constructor_shape = analyzer["constructor_shape"]
function_name = analyzer["function_name"]
original = parse_forms(Path(os.environ["EPSILON"]).read_text())
expanded = parse_forms(family_expansion.decode("ascii"))
count_names = {
    "epsilon_expression_list_count",
    "epsilon_parameter_list_count",
    "epsilon_name_list_count",
}

def selected(form):
    name = function_name(form)
    return (
        recursive_list(form) and constructor_shape(form) == "0/2"
    ) or (
        is_def(form) and (
            name.startswith("epsilon_reverse_")
            and name != "epsilon_reverse_control_references"
            or name in count_names
        )
    )

def rename(tree, old, new):
    if isinstance(tree, list):
        return [rename(item, old, new) for item in tree]
    return new if tree == old else tree

wanted = {str(form.tree[1]): form.tree for form in original if selected(form)}
actual = {str(form.tree[1]): form.tree for form in expanded}
for name in count_names:
    actual[name] = rename(actual[name], "input", wanted[name][2][0][0])
if wanted != actual or len(actual) != 52:
    raise SystemExit("derived Epsilon list family is not alpha-equivalent")

if 292 + 27 - 254 != 65:
    raise SystemExit("derived-list break-even arithmetic changed")
PY

echo "Delta boundary experiment: all five proposed mechanisms fail current earned-feature tests"
