#!/usr/bin/env python3
"""Independent table-driven reference; only --freeze authors expected bytes.

The table describes (phase, opcode)->(phase, decision), separate from the
Omega state graph. Deadline and ownership gates are applied before the table.
No floating arithmetic: equality of full stdout and process exit, tolerance 0.
"""
from pathlib import Path
import argparse
import hashlib
import json

HEADER = "IND22 offline laboratory protocol; deadline=5; capacity=64\n"
LABELS = ["ALLOW acquire", "ALLOW load", "ALLOW start interlock=closed",
          "ALLOW advance deadline=pending", "ALLOW finish lease=released",
          "ABORT cancel lease=retained", "ALLOW recover lease=released",
          "ALLOW lid", "DENY resource_owner", "DENY phase",
          "DENY interlock=open", "ABORT timeout lease=retained", "DENY input",
          "ABORT interlock=open lease=retained"]
TRANSITIONS = {(1, "L"): (2, 1), (2, "S"): (3, 2), (3, "F"): (5, 4),
               (1, "C"): (4, 5), (2, "C"): (4, 5), (3, "C"): (4, 5),
               (4, "R"): (0, 6)}


def simulate(content):
    owner = phase = elapsed = completed = aborted = 0
    closed = False
    output = HEADER
    lines = content.split(b"\n")
    if lines[-1] == b"":
        lines.pop()
    if not content:
        lines = []
    for index, line in enumerate(lines):
        if index == 64:
            return output + "BUDGET_EXHAUSTED\n", 65
        decision = 12
        if len(line) == 2 and 48 <= line[1] <= 57:
            operation = chr(line[0])
            argument = line[1] - 48
            if operation == "O" and argument in (0, 1):
                closed = bool(argument)
                decision = 7
                if not closed and phase == 3:
                    phase, decision = 4, 13
                    aborted += 1
            elif operation == "T" and argument > 0:
                decision = 9
                if phase == 3:
                    elapsed += argument
                    decision = 3
                    if elapsed >= 5:
                        elapsed, phase, decision = 5, 4, 11
                        aborted += 1
            elif operation in "ALS FCR".replace(" ", "") and argument in (1, 2):
                decision = 8
                if operation == "A":
                    if owner == 0:
                        owner, phase, elapsed, decision = argument, 1, 0, 0
                elif argument == owner:
                    next_phase, decision = TRANSITIONS.get((phase, operation), (phase, 9))
                    if operation == "S" and phase == 2 and not closed:
                        decision = 10
                    else:
                        phase = next_phase
                        if decision == 2:
                            elapsed = 0
                        elif decision == 4:
                            owner = 0
                            completed += 1
                        elif decision == 5:
                            aborted += 1
                        elif decision == 6:
                            owner = elapsed = 0
        output += (f"{LABELS[decision]} owner={owner:02} phase={phase:02} "
                   f"lid={'closed' if closed else 'open'} tick={elapsed:02} "
                   f"completed={completed:02} aborted={aborted:02}\n")
    return output + "END\n", 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--freeze", action="store_true")
    arguments = parser.parse_args()
    directory = Path(__file__).resolve().parent / "fixtures"
    if arguments.freeze:
        # Adversarial framing and command budget fixtures are frozen as bytes.
        (directory / "eof.txt").write_bytes(b"O1\nA1\nL1\nS1\nF1")
        (directory / "framing.txt").write_bytes(b"O1\r\nA1\x00\n" + b"x" * 100 + b"\nA1\n")
        (directory / "budget.txt").write_bytes(b"O1\n" * 65)
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
