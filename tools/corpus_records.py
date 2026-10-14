"""Corpus outcome records: the text format `corpus_runner` prints and
`tools/corpus_gate.py` keeps as its golden (`tests/omega/corpus_outcomes.txt`).

One line per fixture:

    <tier/group/name> <status> <milliseconds>ms [expected|unexpected]

`expected`/`unexpected` appear only when the fixture's `expected.txt`
fragments were weighed against diagnostics; `key:value` execution facts
from the native leg (`exit:70`, `stdout:match`) follow. A future interpreter
trap producer must emit `interpreter:trapped` only from TerminalExecutionStatus's
Crashed outcome with CrashCause::Trap, without a numeric exit or physical signal;
CrashCause::Abort, exhaustion, errors
and a declined comparison cannot satisfy this fact. Each fact key appears
once: repeated keys are malformed evidence, even when their values agree.
A `regressed-from:<status>[+key:value]*` token records the last recorded
success a row has lost, which is what separates a fixture that never built
here from one that built and no longer does; the gate derives it at
`--record` time and the runner never observes it. A trailing `@<revision>`
token records which checkout measured the row (`+wt` marks a
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
import re
from pathlib import Path

HEADER = """\
# Omega corpus outcomes, written by `python tools/corpus_gate.py --record`.
# One fixture per line: <tier/group/name> <status> <milliseconds>ms, then
# `expected` or `unexpected` when its expected.txt fragments were weighed,
# then any execution facts such as exit:70 or stdout:match, then
# `regressed-from:<status>[+fact]*` when the row has lost a success it used
# to record, then `@<revision>` recording which commit measured the row
# (`+wt` = dirty tree; rows measured before stamping have no token).
# Tab-indented lines under a fixture are its diagnostics, in order.
# A `# context: {...}` line records the conditions of the --record run.
"""

CONTEXT_PREFIX = "# context: "

# A row's status column says what the compiler did today. It cannot also say
# what the fixture used to do, so a native row that stops building reads
# identically to one that never built here -- and `--record` had no way to
# write the loss down at all, because an unmet authored oracle refused the
# whole write. `regressed-from:` is that missing sentence: provenance the gate
# derives when it replaces a row, naming the last success the fixture recorded
# and that success's execution facts. The row keeps today's status and
# diagnostics, so writing a loss is a different act from pinning it as
# expected -- the gate still reports the unmet oracle and still exits
# `regressed`. Only a success can be lost: the token carries `built` or
# `checked`, never a refusal.
REGRESSION_PREFIX = "regressed-from:"
SUCCESS_STATUSES = ("built", "checked")


def format_regression(outcome: dict) -> str:
    """The `regressed-from:` token naming one lost success outcome."""
    if outcome["status"] not in SUCCESS_STATUSES:
        raise ValueError(
            f"lost outcome is not a success: {outcome['status']!r}")
    token = outcome["status"]
    for key, value in outcome.get("facts", {}).items():
        if not key or not value or any(
                character in key or character in value
                for character in "+:" + " \t\r\n"):
            raise ValueError(f"lost fact {key!r} is not token-safe")
        token += f"+{key}:{value}"
    return REGRESSION_PREFIX + token


def parse_regression(token: str) -> dict:
    """The lost outcome a `regressed-from:` token names.

    A token carrying any other status is malformed rather than a weaker
    claim: a row must not be able to assert a regression from a refusal.
    """
    status, *pairs = token.split("+")
    if status not in SUCCESS_STATUSES:
        raise ValueError(
            f"regressed-from status must be a success: {status!r}")
    facts: dict[str, str] = {}
    for pair in pairs:
        key, separator, value = pair.partition(":")
        if not separator or not key or not value:
            raise ValueError(f"malformed regressed-from fact {pair!r}")
        if key in facts:
            raise ValueError(f"duplicate regressed-from fact {key!r}")
        facts[key] = value
    return {"status": status, "facts": facts}


def carried_regression(golden_record, observed: dict):
    """The last recorded success `observed` is losing, or None.

    Derived from the row being replaced, so it maintains itself in both
    directions: a fixture that succeeds again drops the token, which is what
    makes a later repair a visible move, and a row that keeps failing
    carries the ORIGINAL success forward instead of re-pointing at
    yesterday's failure. A row that still records a weaker success -- `built`
    down to `checked` -- carries no token, because its own status column
    already tells that story.
    """
    if observed["status"] in SUCCESS_STATUSES or golden_record is None:
        return None
    if golden_record["status"] in SUCCESS_STATUSES:
        return {"status": golden_record["status"],
                "facts": dict(golden_record.get("facts", {}))}
    return golden_record.get("regressed_from")


def lost_successes(records: list) -> list:
    """Rows recording a success they no longer achieve."""
    return [record for record in records if record.get("regressed_from")]


def pinned_off_target(records: list) -> list:
    """Rows the golden pins in a status their tier contradicts: a pass or
    run row recorded without a success, or a fail row recorded as one.
    `regressed_from` says whether a success was recorded before, but its
    absence does not make the row acceptable -- a pass fixture pinned
    `rejected` is a pinned failure whether or not it ever built here."""
    return [record for record in records
            if (record["tier"] in ("pass", "run")
                and record["status"] not in SUCCESS_STATUSES)
            or (record["tier"] == "fail"
                and record["status"] in SUCCESS_STATUSES)]


def _outcome_rank(record) -> int:
    """How much of its tier's success a row reached: a fail row succeeds by
    rejecting, a pass or run row by checking and more by building."""
    if record is None:
        return -1
    if record["tier"] == "fail":
        return 1 if record["status"] == "rejected" else 0
    return {"checked": 1, "built": 2}.get(record["status"], 0)


def compare_records(base: list, head: list) -> dict:
    """What `head` lost and gained against `base`, fixture by fixture.

    Both sides are fresh runs of one selection, so this compares measured
    behavior, not a golden that may be commits old. A loss is a fixture
    absent from `head`, a weaker outcome for its tier, an expected fragment
    no longer met, or an execution fact (`exit`, `stdout`, ...) that changed
    or disappeared. Gains are the same moves in the other direction; timing
    and diagnostic text are neither.
    """
    base_rows = {record["fixture"]: record for record in base}
    head_rows = {record["fixture"]: record for record in head}
    losses, gains = [], []
    for fixture in sorted(base_rows.keys() | head_rows.keys()):
        old, new = base_rows.get(fixture), head_rows.get(fixture)
        if new is None:
            losses.append(f"{fixture}: absent from head")
            continue
        if old is None:
            gains.append(f"{fixture}: new {new['status']}")
            continue
        old_rank, new_rank = _outcome_rank(old), _outcome_rank(new)
        move = f"{fixture}: {old['status']} -> {new['status']}"
        if new_rank < old_rank:
            losses.append(move)
        elif new_rank > old_rank:
            gains.append(move)
        if old["expected_satisfied"] is True and new["expected_satisfied"] is not True:
            losses.append(f"{fixture}: expected fragments no longer met")
        elif new["expected_satisfied"] is True and old["expected_satisfied"] is not True:
            gains.append(f"{fixture}: expected fragments now met")
        old_facts, new_facts = old.get("facts", {}), new.get("facts", {})
        for key in sorted(old_facts.keys() | new_facts.keys()):
            before, after = old_facts.get(key), new_facts.get(key)
            if before == after:
                continue
            change = f"{fixture}: {key} {before or '-'} -> {after or '-'}"
            (losses if before is not None else gains).append(change)
    return {"losses": losses, "gains": gains,
            "fixtures": len(base_rows.keys() | head_rows.keys())}


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
    diagnostics, regressed_from (the lost success outcome, or None) and
    recorded_at (the revision stamp, or None on legacy rows)."""
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
        regressed_from = None
        for index, token in enumerate(fields[3:], 3):
            if (token in ("expected", "unexpected") and verdict is None
                    and not facts and regressed_from is None):
                verdict = token
            elif token.startswith("@"):
                if index != len(fields) - 1 or not token[1:]:
                    raise ValueError(
                        f"line {number}: @{token[1:]} must be the last token")
                recorded_at = token[1:]
            elif token.startswith(REGRESSION_PREFIX):
                # Derived provenance, not an execution fact: it never joins
                # `facts`, so a reader counting observations and the gate
                # comparing rows both stay blind to it.
                if regressed_from is not None:
                    raise ValueError(
                        f"line {number}: duplicate regressed-from token")
                try:
                    regressed_from = parse_regression(
                        token[len(REGRESSION_PREFIX):])
                except ValueError as error:
                    raise ValueError(f"line {number}: {error}") from error
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
            "regressed_from": regressed_from,
            "recorded_at": recorded_at,
        })
    return records


def render(records: list[dict], context=None) -> str:
    lines = [HEADER.rstrip("\n")]
    if context is not None:
        lines.append(CONTEXT_PREFIX + json.dumps(context, ensure_ascii=True,
                                                 sort_keys=True))
    for record in records:
        lines.extend(record_lines(record))
    return "\n".join(lines) + "\n"


def record_lines(record: dict) -> list[str]:
    """One fixture's row and its tab-indented diagnostics."""
    header = f"{record['fixture']} {record['status']} {record['millis']}ms"
    if record["expected_satisfied"] is not None:
        header += " expected" if record["expected_satisfied"] else " unexpected"
    for key, value in record.get("facts", {}).items():
        header += f" {key}:{value}"
    if record.get("regressed_from") is not None:
        header += " " + format_regression(record["regressed_from"])
    if record.get("recorded_at") is not None:
        header += f" @{record['recorded_at']}"
    return [header] + ["\t" + escape(diagnostic) for diagnostic in record["diagnostics"]]


def splice(text: str, records: list[dict]) -> str:
    """Write `records` into an existing golden's text and change nothing else.

    A filtered or sharded `--record` measures a few fixtures. Re-rendering the
    whole file from parsed rows also reorders rows, drops comments and
    rewrites the context line, so two sessions recording disjoint fixtures
    conflicted on every rebase. Here each recorded fixture replaces its own
    row block in place; a new fixture goes before the first row that sorts
    after it. Comments, context lines and every other row stay byte-for-byte.
    """
    blocks: list[list[str]] = []
    for line in text.splitlines():
        if line.startswith("\t") and blocks and block_fixture(blocks[-1]):
            blocks[-1].append(line)
        else:
            blocks.append([line])
    for record in sorted(records, key=lambda record: record["fixture"]):
        fixture = record["fixture"]
        replacement = record_lines(record)
        existing = [index for index, block in enumerate(blocks)
                    if block_fixture(block) == fixture]
        if existing:
            blocks[existing[0]] = replacement
            for index in reversed(existing[1:]):
                del blocks[index]
            continue
        position = next((index for index, block in enumerate(blocks)
                         if (block_fixture(block) or "") > fixture), len(blocks))
        blocks.insert(position, replacement)
    return "\n".join(line for block in blocks for line in block) + "\n"


def block_fixture(block: list[str]):
    """The fixture a row block names, or None for a comment or blank line."""
    first = block[0]
    if not first.strip() or first.startswith("#") or first.startswith("\t"):
        return None
    return first.split(" ", 1)[0]


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
    when the revision or its distance is unavailable. Pure otherwise; callers
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
        text += f", {report['unknown_revisions']} revision distance(s) unavailable"
    oldest = report["oldest"]
    if oldest:
        text += (f", oldest stamped row {oldest['commits_behind']} commits "
                 f"behind HEAD (@{oldest['revision']})")
    return text


# This marker is an explicit producer refusal, never a guess from diagnostic
# prose. A reviewed roster identifies each allowed capability/task/route. The
# oracle is still present on disk and applies again as soon as execution works.
MIGRATION_MARKER = re.compile(
    r"^unimplemented: capability=([a-z][a-z0-9-]*) "
    r"task=([A-Z][A-Z0-9-]*) route=(compile|native|interpreter): .+$")


def migration_roster(path, corpus):
    """Load reviewed designations; no supplied roster designates nothing.

    Every entry names one existing canary, capability, owning task and route.
    Fail fixtures cannot be designated: their refusals are semantic controls.
    A real authored oracle must remain beside the canary.
    """
    if path is None:
        return {}
    entries = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(entries, list):
        raise ValueError("migration roster must be a list")
    roster = {}
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {
                "fixture", "capability", "task", "route"}:
            raise ValueError("migration designation requires fixture/capability/task/route")
        fixture = entry["fixture"]
        # Fixture IDs are whitespace-delimited records and newline-separated
        # runner selections. Reject spelling that could change identity during
        # either transport, before consulting host-specific filename behavior.
        if not isinstance(fixture, str) or any(
                character.isspace() or ord(character) < 32 or 127 <= ord(character) <= 159
                for character in fixture):
            raise ValueError("invalid migration fixture path")
        components = fixture.split("/")
        if (len(components) < 3 or "\\" in fixture or ":" in fixture
                or any(component in ("", ".", "..") for component in components)):
            raise ValueError("invalid migration fixture path")
        if components[0] not in ("pass", "run", "pending"):
            raise ValueError("migration fixture must be pass/run/pending")
        directory = Path(corpus) / fixture
        if not (directory / "main.omg").is_file():
            raise ValueError(f"migration canary missing: {fixture}")
        if not ((directory / "expected_stdout.txt").is_file()
                or (directory / "expected_stderr.txt").is_file()
                or any(directory.glob("expected_native_exit*.txt"))):
            raise ValueError(f"migration canary lacks authored oracle: {fixture}")
        marker = (f"unimplemented: capability={entry['capability']} "
                  f"task={entry['task']} route={entry['route']}: missing")
        if not MIGRATION_MARKER.fullmatch(marker):
            raise ValueError(f"invalid migration capability/task/route: {fixture}")
        key = (fixture, entry["route"])
        if key in roster:
            raise ValueError(f"duplicate migration designation: {key}")
        roster[key] = entry
    return roster


def migration_gap_route(record):
    """Recorded debt route, never a successful execution or admission."""
    facts = record.get("facts", {})
    if (record["status"] == "unimplemented"
            or facts.get("interpreter") == "unimplemented"):
        return facts.get("migration-route")
    return None


def classify_migration(records, roster):
    """Annotate only exact designated producer refusals before oracle checks.

    Mixed checker diagnostics, generic failures, timeout/crash records and
    partially executed routes cannot become gaps. No matching refusal leaves
    a designation subject to its unchanged exit/stdout oracle.
    """
    for record in records:
        diagnostics = record.get("diagnostics", [])
        matches = [MIGRATION_MARKER.fullmatch(text) for text in diagnostics]
        gap = None
        if matches and all(matches):
            identities = {match.groups() for match in matches}
            if len(identities) == 1:
                capability, task, route = next(iter(identities))
                entry = roster.get((record["fixture"], route))
                if entry and (entry["capability"], entry["task"]) == (capability, task):
                    facts = record["facts"]
                    blocked_keys = (("interpreter-exit", "interpreter-stdout", "interpreter-signal", "interpreter-stderr")
                                    if route == "interpreter" else
                                    ("exit", "stdout", "signal", "stderr"))
                    if route == "compile":
                        blocked_keys += ("interpreter-exit", "interpreter-stdout", "interpreter-signal", "interpreter-stderr")
                    expected_status = (("checked", "built") if route == "interpreter"
                                       else ("rejected", "unimplemented"))
                    # A declined/unchecked/disagreeing execution is already
                    # an observation, not a missing pre-execution capability.
                    observed_route = facts.get("interpreter")
                    if (record["status"] in expected_status
                            and not any(key in facts for key in blocked_keys)
                            and observed_route in (None, "unimplemented")
                            and record["tier"] != "fail"):
                        gap = (capability, task, route)
        if gap is None:
            if (record["facts"].get("interpreter") == "unimplemented"
                    or record["status"] == "unimplemented"):
                raise ValueError(f"unsubstantiated migration gap: {record['fixture']}")
            continue
        capability, task, route = gap
        facts = record["facts"]
        facts.update({"migration-capability": capability, "migration-task": task,
                      "migration-route": route})
        if route == "interpreter":
            facts["interpreter"] = "unimplemented"
        else:
            record["status"] = "unimplemented"
    return records


def missing_migration_canaries(records, roster, fixture_filter="", shard=""):
    """Do not let a pending designation disappear from the current runner.

    Match its existing substring filter and stable FNV-1a shard contract. This
    is coverage checking only: no copying canaries into another tier, alternate
    compiler runner, or marking absent executions as diagnosed missing semantics.
    """
    fragments = [value.strip() for value in fixture_filter.split(",") if value.strip()]
    selected_shard = None
    if shard:
        try:
            tokens = [part.strip() for part in shard.split("/")]
            if len(tokens) == 2 and all(re.fullmatch(r"\+?[0-9]+", part) for part in tokens):
                index, count = (int(part) for part in tokens)
                if 1 <= count < 2 ** 64 and 0 <= index < count:
                    selected_shard = (index, count)
        except ValueError:
            pass  # The runner also treats a malformed shard as unsharded.
    observed = {record["fixture"] for record in records}
    missing = []
    for fixture in sorted({key[0] for key in roster}):
        if fragments and not any(fragment in fixture for fragment in fragments):
            continue
        if selected_shard is not None:
            fingerprint = 0xcbf29ce484222325
            for byte in fixture.encode("utf-8"):
                fingerprint = ((fingerprint ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
            index, count = selected_shard
            if fingerprint % count != index:
                continue
        if fixture not in observed:
            missing.append(fixture)
    return missing
