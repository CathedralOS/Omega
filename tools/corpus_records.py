"""Corpus outcome records: the text format `corpus_runner` prints and
`tools/corpus_gate.py` keeps as its golden (`tests/omega/corpus_outcomes.txt`).

One line per fixture:

    <tier/group/name> <status> <milliseconds>ms [expected|unexpected]

`expected`/`unexpected` appear only when the fixture's `expected.txt`
fragments were weighed against diagnostics; `key:value` execution facts
from the native leg (`exit:70`, `stdout:match`) follow. Each fact key appears
once: repeated keys are malformed evidence, even when their values agree.
A trailing `@<revision>` token records which checkout measured the row (`+wt` marks a
dirty tree); a filtered `--record` merges old rows forward, so without the
stamp a `built` row can be days stale. Legacy rows carry no stamp. Each
diagnostic follows on its
own line after one tab, with backslash, newline, carriage return and tab
escaped as `\\\\`, `\\n`, `\\r` and `\\t`. Lines starting with `#` are
comments; a `# context: {...}` comment after the header records the
conditions the recording ran under (worker count, machine parallelism,
load average), which is what makes a later timing comparison meaningful.
Standard library only.
"""

from __future__ import annotations

import json

HEADER = """\
# Omega corpus outcomes, written by `python tools/corpus_gate.py --record`.
# One fixture per line: <tier/group/name> <status> <milliseconds>ms, then
# `expected` or `unexpected` when its expected.txt fragments were weighed,
# then any execution facts such as exit:70 or stdout:match, then
# `@<revision>` recording which commit measured the row (`+wt` = dirty
# tree; rows measured before stamping have no token).
# Tab-indented lines under a fixture are its diagnostics, in order.
# A `# context: {...}` line records the conditions of the --record run.
"""

CONTEXT_PREFIX = "# context: "

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
    expected_satisfied (True, False or None), facts (ordered key to value),
    diagnostics and recorded_at (the revision stamp, or None on legacy
    rows)."""
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
        if len(fields) < 3 or not fields[2].endswith("ms"):
            raise ValueError(f"line {number}: not a fixture record: {line[:120]}")
        fixture, status, millis = fields[:3]
        verdict = None
        facts: dict[str, str] = {}
        recorded_at = None
        for index, token in enumerate(fields[3:], 3):
            if token in ("expected", "unexpected") and verdict is None and not facts:
                verdict = token
            elif token.startswith("@"):
                if index != len(fields) - 1 or not token[1:]:
                    raise ValueError(
                        f"line {number}: @{token[1:]} must be the last token")
                recorded_at = token[1:]
            elif ":" in token:
                key, value = token.split(":", 1)
                # Overwriting an earlier exit/stdout fact could turn a failed
                # runtime oracle into a match before the gate ever sees it.
                if key in facts:
                    raise ValueError(f"line {number}: duplicate fact {key!r}")
                facts[key] = value
            else:
                raise ValueError(f"line {number}: unknown token {token!r}")
        records.append({
            "fixture": fixture,
            "tier": fixture.split("/", 1)[0],
            "status": status,
            "millis": int(millis[:-2]),
            "expected_satisfied": None if verdict is None else verdict == "expected",
            "facts": facts,
            "diagnostics": [],
            "recorded_at": recorded_at,
        })
    return records


def render(records: list[dict], context=None) -> str:
    lines = [HEADER.rstrip("\n")]
    if context is not None:
        lines.append(CONTEXT_PREFIX + json.dumps(context, ensure_ascii=True,
                                                 sort_keys=True))
    for record in records:
        header = f"{record['fixture']} {record['status']} {record['millis']}ms"
        if record["expected_satisfied"] is not None:
            header += " expected" if record["expected_satisfied"] else " unexpected"
        for key, value in record.get("facts", {}).items():
            header += f" {key}:{value}"
        if record.get("recorded_at") is not None:
            header += f" @{record['recorded_at']}"
        lines.append(header)
        lines.extend("\t" + escape(diagnostic) for diagnostic in record["diagnostics"])
    return "\n".join(lines) + "\n"


def read(path) -> list[dict]:
    with open(path, encoding="utf-8") as handle:
        return parse(handle.read())


def context_from(text: str):
    """The recording context a `--record` wrote as a header comment, or
    None when the golden predates context stamping (or it is unreadable).
    Kept separate from parse(): rows and run conditions have different
    callers and lifetimes."""
    for line in text.splitlines():
        if line.startswith(CONTEXT_PREFIX):
            try:
                context = json.loads(line[len(CONTEXT_PREFIX):])
            except ValueError:
                return None
            return context if isinstance(context, dict) else None
        if line and not line.startswith("#"):
            return None
    return None


def read_context(path):
    with open(path, encoding="utf-8") as handle:
        return context_from(handle.read())


# Retired diagnostic prefixes and the spelling that replaced them. The
# table exists so a pure wording change in a compiler diagnostic stops
# reading as behavior movement; add an entry in the same commit that
# renames a diagnostic prefix.
DIAGNOSTIC_PREFIX_ALIASES = (
    ("native-artifact Terminal production failed: ",
     "terminal-artifact production failed: "),
)


def normalized_diagnostic(text: str) -> str:
    for retired, current in DIAGNOSTIC_PREFIX_ALIASES:
        if text.startswith(retired):
            return current + text[len(retired):]
    return text


def normalized_diagnostics(diagnostics: list) -> list:
    return [normalized_diagnostic(text) for text in diagnostics]


def provenance(records: list[dict], commits_between) -> dict:
    """Staleness report over the rows' recorded_at stamps.

    `commits_between(revision)` returns commits between that revision and
    the measuring head (e.g. `git rev-list --count <rev>..HEAD`) or None
    when the revision is not in local history. Pure otherwise; callers
    inject the lookup. Informational only — it never gates anything.
    """
    stamped = legacy = 0
    behind = {}
    unknown = set()
    for record in records:
        stamp = record.get("recorded_at")
        if not stamp:
            legacy += 1
            continue
        stamped += 1
        revision = stamp.split("+", 1)[0]
        if revision not in behind:
            behind[revision] = commits_between(revision)
        if behind[revision] is None:
            unknown.add(revision)
    known = {revision: count for revision, count in behind.items()
             if count is not None}
    oldest = None
    if known:
        revision = max(known, key=known.get)
        oldest = {"revision": revision, "commits_behind": known[revision]}
    return {"stamped": stamped, "legacy": legacy,
            "unknown_revisions": len(unknown), "oldest": oldest}


def provenance_line(report: dict) -> str:
    text = (f"golden provenance: {report['stamped']} rows stamped, "
            f"{report['legacy']} legacy (unstamped)")
    if report["unknown_revisions"]:
        text += f", {report['unknown_revisions']} unknown-revision row(s)"
    oldest = report["oldest"]
    if oldest:
        text += (f", oldest stamped row {oldest['commits_behind']} commits "
                 f"behind HEAD (@{oldest['revision']})")
    return text
