"""Observe runtime results independently of erased evidence and inspect its ABI lane."""
import argparse
import csv
import hashlib
import json
import re
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / "samples/cli/erased_array_evidence"


def invoke(binary, *arguments):
    result = subprocess.run([str(binary), *map(str, arguments)], cwd=ROOT,
                            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=180)
    print(json.dumps({"command": [str(binary), *map(str, arguments)],
                      "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}), flush=True)
    return result


def runtime(binary, native_artifact=None):
    expected = int((PROJECT / "expected_native_exit.txt").read_text())
    prepared_digest = hashlib.sha256(native_artifact.read_bytes()).hexdigest() if native_artifact else None
    # Retain captured outputs and the exact fresh native program for review.
    directory = tempfile.mkdtemp(prefix="erased-array-runtime-", dir=ROOT / "build")
    result = invoke(binary, "run", "--both", "--keep", "--input", PROJECT / "input.txt",
                    "--outputs", directory, PROJECT / "main.omg")
    assert result.returncode == 0, "native/Terminal execution failed"
    with (Path(directory) / "results.tsv").open() as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    print(json.dumps({"observations": rows, "expected_exit": expected}), flush=True)
    assert len(rows) == 1, "missing or extra runtime observation"
    row = rows[0]
    assert row["interpreter"] == "agrees", "Terminal did not agree"
    assert int(row["native"]) == expected, "native result differs from authored oracle"
    assert int(row["interpreter_exit"]) == expected, "Terminal result differs from authored oracle"
    for suffix in ("stdout", "stderr", "terminal.stdout", "terminal.stderr"):
        assert (Path(directory) / f"input.{suffix}").read_bytes() == b"", suffix
    if native_artifact:
        kept = [line.removeprefix("build dir kept: ") for line in result.stderr.splitlines()
                if line.startswith("build dir kept: ")]
        assert len(kept) == 1, "native publication directory missing"
        fresh = Path(kept[0]) / native_artifact.name
        actual_digest = hashlib.sha256(fresh.read_bytes()).hexdigest()
        print(json.dumps({"prepared_native": str(native_artifact), "prepared_sha256": prepared_digest,
                          "fresh_native": str(fresh), "fresh_sha256": actual_digest,
                          "captured_outputs": directory}), flush=True)
        assert actual_digest == prepared_digest, "fresh native differs from declared artifact"


def sources(binary):
    relevance = ROOT / "tests/omega"
    positives = sorted((relevance / "pass/relevance").glob("erased_array*/main.omg"))
    negatives = sorted((relevance / "fail/relevance").glob("erased_array*/main.omg"))
    negatives += [relevance / "fail/relevance" / name / "main.omg" for name in (
        "erased_nonscalar_parameter", "erased_parameter_runtime_read", "erased_mutable_parameter")]
    assert positives and negatives
    for path in positives:
        assert invoke(binary, "--check", path).returncode == 0, str(path)
    for path in negatives:
        expected = [line for line in path.with_name("expected.txt").read_text().splitlines() if line]
        assert expected, str(path)
        result = invoke(binary, "--check", path)
        assert result.returncode == 1 and all(text in result.stderr for text in expected), str(path)
    print(json.dumps({"positive_count": len(positives), "negative_count": len(negatives)}))


def inspection(binary):
    # Public inspection reads the produced Terminal module, not source syntax.
    for name in ("erased_array_calls", "erased_array_nested", "erased_array_transition"):
        path = ROOT / "tests/omega/pass/relevance" / name / "main.omg"
        result = invoke(binary, "inspect-terminal", "--machine", "Main::main", path)
        assert result.returncode == 0, name
        text = result.stdout
        assert 'Array { type_identity: "array(' in text, "array evidence missing"
        assert "literal(2)" in text and "Signed(7)" in text and "Signed(8)" in text, name
        signatures = [line for line in text.splitlines() if line.startswith("signature ")]
        proof_signatures = [line for line in signatures if not line.endswith("erased_proofs=[]")]
        assert proof_signatures, "erased formal roster missing"
        for signature in proof_signatures:
            assert signature.split(" erased_scalars=")[0].count("ValueDeclaration {") == 1, signature
        runtime_types = [line for line in text.splitlines() if line.startswith("type ")]
        assert len(runtime_types) == 1, runtime_types
        # Package-owned nominal identity is canonical; erased array evidence must
        # still add neither a runtime type nor another Main storage field.
        identity = runtime_types[0].split(" identity=", 1)[1].split(" shape=", 1)[0].replace("\\", "")
        owner = re.search(r"package-owner\(32:([a-f0-9]{64})\)", identity)
        assert owner and identity == f"named(name(nominal(package-owner(32:{owner[1]}),path(Main))))", runtime_types
        assert runtime_types[0].endswith(" shape=record(fields=1)"), runtime_types
        if name in ("erased_array_calls", "erased_array_transition"):
            assert "Formal { position: 0 }" in text, "forwarded evidence missing"
        if name == "erased_array_calls":
            assert 'field_identity: "Proof::table", term: Array' in text
            assert "elements: [Scalar(Value { id:" in text, "scalar local evidence lost"
        print(json.dumps({"inspection": name, "runtime_types": runtime_types,
                          "proof_signatures": proof_signatures}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("runtime", "sources", "inspection"))
    parser.add_argument("binary", type=Path)
    parser.add_argument("--native-artifact", type=Path)
    arguments = parser.parse_args()
    if arguments.mode == "runtime":
        runtime(arguments.binary.resolve(), arguments.native_artifact)
    else:
        {"sources": sources, "inspection": inspection}[arguments.mode](arguments.binary.resolve())
