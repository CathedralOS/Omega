#!/usr/bin/env python3
"""Independent list-driven reference; only --freeze authors expected bytes.

The registry is a Python list scanned with str.count and str.index, separate
from the Omega state-graph scan. No floating arithmetic: equality of full
stdout and process exit, tolerance 0.
"""
from pathlib import Path
import argparse
import hashlib
import json

HEADER = "NEW064 research record linkage; registry=8; commands=64\n"
CAPACITY = 8
BUDGET = 64


def simulate(content):
    keys = []
    registered = observations = unique = ambiguous = unlinked = 0
    rejected = capacity_denied = 0
    registry_checksum = decisions_checksum = 0
    output = HEADER
    lines = content.split(b"\n")
    if lines[-1] == b"":
        lines.pop()
    if not content:
        lines = []
    for index, line in enumerate(lines):
        if index == BUDGET:
            return output + "BUDGET_EXHAUSTED\n", 65
        valid = (len(line) == 3 and 48 <= line[1] <= 57
                 and 48 <= line[2] <= 57)
        opcode = chr(line[0]) if valid else ""
        key = (line[1] - 48) * 10 + (line[2] - 48) if valid else 0
        if not valid or opcode not in "ROA":
            rejected += 1
            output += "DENY input\n"
            continue
        if opcode == "R":
            if len(keys) < CAPACITY:
                registry_checksum = (registry_checksum + (len(keys) + 1) * key) % 97
                output += f"ALLOW register slot={len(keys):02} key={key:02}\n"
                keys.append(key)
                registered += 1
            else:
                capacity_denied += 1
                output += f"DENY capacity key={key:02} registered={registered:02}\n"
        elif opcode == "O":
            observations += 1
            found = keys.count(key)
            decisions_checksum = (decisions_checksum + observations * (1 if found == 1 else 2 if found > 1 else 3)) % 97
            if found == 1:
                unique += 1
                output += f"LINK unique slot={keys.index(key):02} key={key:02}\n"
            elif found > 1:
                ambiguous += 1
                output += f"LINK ambiguous key={key:02} matches={found:02}\n"
            else:
                unlinked += 1
                output += f"UNLINKED key={key:02}\n"
        else:
            output += (f"AUDIT tag={key:02} registered={registered:02} "
                       f"observations={observations:02} unique={unique:02} "
                       f"ambiguous={ambiguous:02} unlinked={unlinked:02} "
                       f"rejected={rejected:02} capacity={capacity_denied:02} "
                       f"registry={registry_checksum:02} decisions={decisions_checksum:02}\n")
    return output + "END\n", 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--freeze", action="store_true")
    arguments = parser.parse_args()
    directory = Path(__file__).resolve().parent / "fixtures"
    if arguments.freeze:
        # Adversarial framing and command budget fixtures are frozen as bytes.
        (directory / "eof.txt").write_bytes(b"R42\nR07\nO42\nA00\nO07")
        (directory / "framing.txt").write_bytes(b"R42\r\nO42\x00\n" + b"x" * 100 + b"\nR42\n")
        (directory / "budget.txt").write_bytes(b"R01\n" * 65)
    manifest = {}
    for fixture in sorted(directory.glob("*.txt")):
        output, status = simulate(fixture.read_bytes())
        expected = fixture.with_suffix(".stdout")
        if arguments.freeze:
            expected.write_text(output)
            fixture.with_suffix(".exit").write_text(str(status) + "\n")
        else:
            assert expected.read_text() == output, fixture
            assert int(fixture.with_suffix(".exit").read_text()) == status, fixture
        manifest[fixture.name] = {"input_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
                                  "stdout_sha256": hashlib.sha256(output.encode()).hexdigest(),
                                  "exit": status}
    if arguments.freeze:
        (directory / "oracle_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    else:
        assert json.loads((directory / "oracle_manifest.json").read_text()) == manifest
    print(f"{len(manifest)} frozen oracles verified; tolerance=0")

if __name__ == "__main__":
    main()
