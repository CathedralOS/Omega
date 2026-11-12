#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

REPO = Path(
    subprocess.check_output(
        ["git", "-C", str(Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"],
        text=True,
    ).strip()
)


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def ledger_status(fixture: str) -> str | None:
    for line in (REPO / "tests/omega/corpus_outcomes.txt").read_text().splitlines():
        if line.startswith(fixture + " "):
            return line.split()[1]
    return None


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    fixture, status = parse_header(line)
    tier = fixture.split("/")[0]
    prior = ledger_status(fixture)
    if prior is None:
        return 125
    flags = [p for p in line.split()[3:] if not p.startswith("@") and not p.endswith("ms")]
    is_fail = tier == "fail"
    if is_fail:
        if prior in ("rejected", "incomplete"):
            bad = status not in ("rejected", "incomplete") or "unexpected" in flags
        else:
            bad = False
    else:
        rank = {"built": 2, "checked": 1}
        bad = rank.get(status, 0) < rank.get(prior, 0) or "unexpected" in flags
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
