"""Execute this fact row and check its authored exits and rejection reasons."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--only", help="comma-separated positive place names")
    parser.add_argument("--controls", action="store_true")
    options = parser.parse_args()
    root = Path(__file__).resolve().parent
    options.output.mkdir(parents=True, exist_ok=True)
    places = (options.only.split(",") if options.only else [
        "requirement_probe", "local", "field", "parameter", "call_result",
        "match_join", "const_name", "other_state", "other_machine",
        "target_body", "selection",
    ])
    projects = []
    for place in places:
        projects.append((place, root / place, False))
        if (root / place / "invalid").is_dir():
            projects.append((place + "-invalid", root / place / "invalid", True))
    if options.controls:
        repository = root.parents[3]
        projects.extend((folder.name, folder, True) for folder in sorted(
            (repository / "tests/omega/fail/domains").glob("named_issuer_*")))
    records = []
    for name, project, invalid in projects:
        source = project / "main.omg"
        command = [str(options.cli.resolve()), "run", "--both", "--keep", str(source)]
        result = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True,
                                text=True, timeout=180)
        output = result.stdout + result.stderr
        (options.output / (name + ".log")).write_text(output)
        expected = (project / "expected.txt").read_text().strip() if invalid else None
        passed = (result.returncode == 200 and expected in output if invalid else
                  result.returncode == 0 and "native exit 0" in output
                  and "interp exit: 0" in output and "DECLINED" not in output)
        artifacts = []
        for kept in re.findall(r"build dir kept: (.+)", output):
            artifacts.extend({"path": str(path), "sha256": digest(path)}
                             for path in sorted(Path(kept).rglob("*")) if path.is_file())
        record = {"name": name, "source": str(source), "source_sha256": digest(source),
                  "command": command, "exit": result.returncode, "expected": expected,
                  "passed": passed, "artifacts": artifacts}
        records.append(record)
        print(json.dumps(record), flush=True)
        (options.output / "results.json").write_text(json.dumps(records, indent=2) + "\n")
    return 0 if all(record["passed"] for record in records) else 1


if __name__ == "__main__":
    raise SystemExit(main())
