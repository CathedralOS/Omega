"""Check independent contract and native/Terminal oracles through the public CLI."""

import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def invoke(command):
    result = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True,
                            text=True, timeout=120)
    print(json.dumps({"command": command, "exit": result.returncode,
                      "stdout": result.stdout, "stderr": result.stderr}), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--omega", required=True)
    parser.add_argument("--mode", choices=["positive", "negative", "execution"], required=True)
    arguments = parser.parse_args()
    sample = Path(__file__).resolve().parent
    repository = sample.parents[3]
    oracle = json.loads((sample / "oracle.json").read_text())
    print(json.dumps({"oracle_sha256": hashlib.sha256((sample / "oracle.json").read_bytes()).hexdigest(),
                      "identities": oracle["identities"]}), flush=True)
    if arguments.mode != "execution":
        tier = "pass" if arguments.mode == "positive" else "fail"
        suffixes = ["exact", "renamed", "composition", "state"] if tier == "pass" else oracle["reject"]
        for suffix in suffixes:
            fixture = repository / "tests/omega" / tier / "proofs" / ("selected_specification_requires_" + suffix)
            result = invoke([arguments.omega, "--check", str(fixture / "main.omg")])
            if tier == "pass":
                assert result.returncode == 0, (suffix, result.stderr)
            else:
                assert result.returncode == 1, (suffix, result.returncode)
                for fragment in (fixture / "expected.txt").read_text().splitlines():
                    assert fragment in result.stderr, (suffix, fragment, result.stderr)
                if suffix == "false_result":
                    assert "cannot prove requires contract" not in result.stderr, result.stderr
        return
    capture = Path(tempfile.mkdtemp(prefix="selected-specification-", dir=repository / "build"))
    inputs = capture / "inputs"
    inputs.mkdir()
    (inputs / "identity.stdin").write_bytes(b"")
    outputs = capture / "outputs"
    result = invoke([arguments.omega, "run", "--both", "--keep", "--inputs", str(inputs),
                     "--outputs", str(outputs), str(sample / "main.omg")])
    assert result.returncode == 0, result.stderr
    table = (outputs / "results.tsv").read_text()
    rows = list(csv.DictReader(table.splitlines(), delimiter="\t"))
    print(json.dumps({"capture": str(capture), "table": table, "rows": rows}), flush=True)
    assert len(rows) == 1 and rows[0]["case"] == "identity", rows
    assert rows[0]["native"] == str(oracle["expected_native_exit"]), rows
    assert rows[0]["interpreter"] == "agrees", rows
    assert rows[0]["interpreter_exit"] == str(oracle["expected_terminal_exit"]), rows
    for suffix in ["stdout", "stderr", "terminal.stdout", "terminal.stderr"]:
        observed = (outputs / ("identity." + suffix)).read_bytes()
        print(json.dumps({"stream": suffix, "bytes": list(observed)}), flush=True)
        assert observed == b"", (suffix, observed)


if __name__ == "__main__":
    main()
