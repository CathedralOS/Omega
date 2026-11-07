#!/usr/bin/env python3
"""Independent W3-01 authored-input oracle; no compiler-derived expectations."""
import argparse, hashlib, json
from pathlib import Path

AUTHORED_INPUTS = ((7, 31), (13, 39))
EXPECTED_EXIT = 70

parser = argparse.ArgumentParser()
parser.add_argument("--sample", type=Path, default=Path("samples/cli/generics/runtime_indexed_carrier"))
parser.add_argument("--native-exit", type=int)
parser.add_argument("--terminal-exit", type=int)
parser.add_argument("--output", type=Path)
options = parser.parse_args()
table = json.loads((options.sample / "oracle.json").read_text())
actual = tuple((row["index"], row["payload"]) for row in table["inputs"])
assert actual == AUTHORED_INPUTS, ("authored input/payload table changed", actual)
assert table["expected_exit"] == EXPECTED_EXIT
assert len({index for index, _ in AUTHORED_INPUTS}) == 2
assert sum(payload for _, payload in AUTHORED_INPUTS) == EXPECTED_EXIT
source = (options.sample / "main.omg").read_text()
# Source audit complements execution: each payload must be checked before exit70.
for index, payload in AUTHORED_INPUTS:
    assert f"preserve({index}, {payload})" in source
assert "first == 31 && second == 39" in source
assert "match indexed.Limit == limit" in source
assert "match indexed.Limit == 9" in source
assert source.count("true -> indexed.value") == 2
assert source.count("false -> 0") == 2
assert "static_control(9)" in source and "control == 9" in source
assert "let indexed: Indexed<9> = Indexed { value: value };" in source
assert "Console::exit_process(console, 70)" in source
assert "Console::exit_process(console, 1)" in source
provided = (options.native_exit, options.terminal_exit)
assert all(code is None for code in provided) or all(code is not None for code in provided), "supply both execution exits"
for boundary, code in zip(("native", "captured_terminal"), provided):
    assert code is None or code == EXPECTED_EXIT, (boundary, code, EXPECTED_EXIT)
result = {"authored_inputs": [{"index": index, "payload": payload} for index, payload in AUTHORED_INPUTS], "expected_exit": EXPECTED_EXIT,
          "oracle_sha256": hashlib.sha256((options.sample / "oracle.json").read_bytes()).hexdigest(),
          "source_sha256": hashlib.sha256((options.sample / "main.omg").read_bytes()).hexdigest(),
          "source_oracle_validated": True, "execution_validated": all(code is not None for code in provided),
          "native_exit": options.native_exit, "captured_terminal_exit": options.terminal_exit}
encoded = json.dumps(result, indent=2) + "\n"
if options.output: options.output.write_text(encoded)
print(encoded, end="")
