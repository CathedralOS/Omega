"""Corpus outcome records: the text format `corpus_runner` prints and
`tools/corpus_gate.py` keeps as its golden (`tests/omega/corpus_outcomes.txt`).

One line per fixture:

    <tier/group/name> <status> <milliseconds>ms [expected|unexpected]

`expected`/`unexpected` appear only when the fixture's `expected.txt`
fragments were weighed against diagnostics. Each diagnostic follows on its
own line after one tab, with backslash, newline, carriage return and tab
escaped as `\\\\`, `\\n`, `\\r` and `\\t`. Lines starting with `#` are
comments. Standard library only.
"""

from __future__ import annotations

HEADER = """\
# Omega corpus outcomes, written by `python tools/corpus_gate.py --record`.
# One fixture per line: <tier/group/name> <status> <milliseconds>ms, then
# `expected` or `unexpected` when its expected.txt fragments were weighed.
# Tab-indented lines under a fixture are its diagnostics, in order.
"""

_ESCAPES = {"\\": "\\\\", "\n": "\\n", "\r": "\\r", "\t": "\\t"}
_UNESCAPES = {"\\": "\\", "n": "\n", "r": "\r", "t": "\t"}


def escape(text: str) -> str:
    return "".join(_ESCAPES.get(character, character) for character in text)


def unescape(text: str) -> str:
    out = []
    characters = iter(text)
    for character in characters:
        if character == "\\":
            following = next(characters, "")
            out.append(_UNESCAPES.get(following, "\\" + following))
        else:
            out.append(character)
    return "".join(out)


def parse(text: str) -> list[dict]:
    """Records as dictionaries: fixture, tier, status, millis,
    expected_satisfied (True, False or None) and diagnostics."""
    records: list[dict] = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        if line.startswith("\t"):
            if not records:
                raise ValueError(f"line {number}: diagnostic before any fixture")
            records[-1]["diagnostics"].append(unescape(line[1:]))
            continue
        fields = line.split(" ")
        if len(fields) not in (3, 4) or not fields[2].endswith("ms"):
            raise ValueError(f"line {number}: not a fixture record: {line[:120]}")
        fixture, status, millis = fields[:3]
        verdict = fields[3] if len(fields) == 4 else None
        if verdict not in (None, "expected", "unexpected"):
            raise ValueError(f"line {number}: unknown verdict {verdict!r}")
        records.append({
            "fixture": fixture,
            "tier": fixture.split("/", 1)[0],
            "status": status,
            "millis": int(millis[:-2]),
            "expected_satisfied": None if verdict is None else verdict == "expected",
            "diagnostics": [],
        })
    return records


def render(records: list[dict]) -> str:
    lines = [HEADER.rstrip("\n")]
    for record in records:
        header = f"{record['fixture']} {record['status']} {record['millis']}ms"
        if record["expected_satisfied"] is not None:
            header += " expected" if record["expected_satisfied"] else " unexpected"
        lines.append(header)
        lines.extend("\t" + escape(diagnostic) for diagnostic in record["diagnostics"])
    return "\n".join(lines) + "\n"


def read(path) -> list[dict]:
    with open(path, encoding="utf-8") as handle:
        return parse(handle.read())
