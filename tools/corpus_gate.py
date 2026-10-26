#!/usr/bin/env python3
"""Corpus outcome gate — the cheap inner-loop regression check.

Builds the `corpus_runner` test target once (a `harness = false` binary that
compiles every `tests/omega/{pass,fail,run}` fixture through the same routes
the canary suite asserts on), runs it, and diffs the emitted per-fixture
outcome records against `tests/omega/corpus_outcomes.txt` (format:
`tools/corpus_records.py`).

The golden records outcome *classes* — `checked`/`rejected`, diagnostic
messages, and whether each fail fixture's expected.txt fragments were
satisfied — so a diff means a fixture's observable behavior moved, not that
an internal assertion's spelling changed. Re-record with `--record` when a
movement is intended (the corpus is not all-green at base; the golden pins
the red as well as the green).

Usage:

    python3 tools/corpus_gate.py            # build runner, run, diff golden
    python3 tools/corpus_gate.py --record   # rebuild + rewrite the golden
    python3 tools/corpus_gate.py --runner target/debug/deps/corpus_runner-*
    python3 tools/corpus_gate.py --list-pinned-regressions  # read stored rows only

Frozen comparisons may supply both `--runner` and `--baseline-runner` with
`--compare REVISION`. The compared checkout still has to be that exact revision,
and both supplied executables retain optimized-profile and fixture-root checks.
This starts no Rust compiler build; the caller records the actual frozen binary
digests and source revisions in its qualification rather than treating the option
as producer provenance.

Subsets — the inner-loop mode:

    python3 tools/corpus_gate.py --filter termination       # one domain
    python3 tools/corpus_gate.py --filter fail/proofs,wire/ # several fragments

Native leg — build every pass and run fixture for this host and execute the
run tier, `*_exit` fixtures and fixtures with `expected_native_exit.txt` (or
a host-specific `expected_native_exit.<target>.txt`, which overrides it),
diffing against this host's golden `tests/omega/corpus_native_<target>.txt`:

    python3 tools/corpus_gate.py --native --filter providers/

An authored `expected_stdout.txt` or `expected_stderr.txt` requires a completed
native execution with its own stream match, independently of any exit oracle.
These promises are checked
before golden comparison and recording, so `--record` cannot bless wrong or
missing output. A declaration-only or foreign-target build supplies no runtime
evidence and cannot satisfy that promise; use a matching bound host.

Recording a regression is not the same act as pinning it as expected. A row
whose fixture no longer compiles produced no result to contradict its oracle,
so `--record` writes today's observation plus a
`regressed-from:<status>[+fact]*` token naming the success the row is losing,
and still exits `regressed` with the unmet promise printed. A row whose
fixture DID run and returned a different exit or different output is
success-shaped — every reader that counts successes counts it — so `--record`
refuses that one outright and names it. Without the token a golden cannot say
whether a fixture never built on this host or built and no longer does, which
is how eight macOS rows went on claiming `exit:70` for programs whose check
leg had started refusing.

A row's `@<sha>` stamp is provenance, not a requirement to run on main.
`--record` retains an unlanded source commit in `refs/corpus-observations/` and
an exportable Git bundle under `build/corpus-source/` before writing the rows.
Publish that bundle with the PR evidence and preserve it through squash/cleanup.
A `+wt` marker still identifies dirty-source observations; preserve their source
patches and replay inputs alongside the bundle. Recording never substitutes a
base revision or requires a postlanding compiler rebuild.

`--native-target <target>` builds for another catalogued target and diffs
against that target's golden. Products execute only where the host runs
them: an Apple-silicon macOS host runs `macos_x86_64` under Rosetta
(`arch -x86_64`); other foreign targets are realized without executing.

    python3 tools/corpus_gate.py --native-target macos_x86_64 --filter host/

Interpreted leg — also execute each runnable fixture through the shared admitted
Terminal program/instance core. For identical selected targets it consumes the
native report's exact Terminal artifact without replaying Build. `--interpret`
defaults to `tests/omega/corpus_interpreted.txt`; `--both` runs native and Terminal
legs together, records whether their exit codes agree, and defaults to
`tests/omega/corpus_both_<target>.txt`. An absent default golden is a usage error;
use an existing explicit golden to inspect runtime fact moves without recording:

    python3 tools/corpus_gate.py --interpret --golden tests/omega/corpus_outcomes.txt --filter _exit
    python3 tools/corpus_gate.py --both --filter run/

`--checked-interpreter` explicitly selects the independent checked-tree source
oracle instead of Terminal execution. It compiles the same reviewed fixture and
uses its selected entry and provisioned receiver; it is never a fallback for a
Terminal refusal. Its separate defaults are `corpus_checked_interpreted.txt` and,
with `--both`, `corpus_checked_both_<target>.txt`. Supply an existing explicit
golden for a bounded replay while these goldens are absent:

    python3 tools/corpus_gate.py --checked-interpreter --golden tests/omega/corpus_outcomes.txt --filter pass/layouts/runtime_callee_integer_at_fitting_write_exit,pass/calls/runtime_guarded_reference_return_crash

An authored stdout oracle must match every requested execution engine.
Interpreter output that differs or was not observed rejects before recording,
even when its exit code agrees with native execution.
Typed Trap observations satisfy trap oracles; decline, exhaustion and host
signals do not. Reviewed `--migration-roster` designations preserve exact named
pre-effect capability debt without changing authored exit/stdout expectations,
and unresolved debt remains nonzero. See the corpus tests README for the bounded
three-control command and `tests/omega/interpreter_migration.json`.

Reading a move — the summary cuts each field at 300 columns, so use
`--write-moves` to get every moved fixture's before/after records in full:

    python3 tools/corpus_gate.py --filter fail/ --write-moves build/moves.json

Measuring your own change against the commit it builds on:

    python3 tools/corpus_gate.py --compare origin/main --filter calls/

`--compare` builds REVISION's runner in its own worktree (`build/cmp`, with
its own build directory) and diffs this tree's run fixture by fixture: a lost
outcome, expected fragment or execution fact exits 1. REVISION's rows are
cached under ignored `build/corpus_baselines/<sha>[-leg].txt`, so later runs
against the same base cost one pass. It answers "what did I move" even while
the checked-in golden disagrees with this checkout over another lane's
in-flight work, and editing this tree cannot leak into the base.

`--filter` (or `OMEGA_CORPUS_FIXTURE_FILTER` directly) matches comma-separated
trimmed substrings against `tier/group/name` and diffs only the fixtures that
ran, against the same golden. `--shard k/N` (or `OMEGA_CORPUS_SHARD`)
selects a deterministic hash-slice of the corpus — stable across fixture
additions, so shards can be recorded or diffed on separate sessions/machines.
The full corpus is scheduled-workload cost, not loop cost — subsets are the
iteration gate; the unfiltered diff is the baseline/scheduled gate.

This is the iteration smoke gate, not the landing gate: validation scopes are
defined by tools/testing.md. Compiler source behavior stays in corpus fixtures;
Rust unit tests belong only to the independent pure owners named there.

Recording caveat: `timeout` records are hardware- and load-relative — a
fixture near `OMEGA_CORPUS_FIXTURE_SECS` can flap between `timeout` and a
real status across hosts or under different pool contention. Known fixtures
get an adaptive cap of max(cap, 3x golden millis), so boundary flap needs a
genuine slowdown rather than a busier box. `OMEGA_CORPUS_JOBS` overrides the
worker count; record and diff at the same jobs value on a quiet machine —
`millis` inflates uniformly under external load, which floods the perf-move
report. An unfiltered `--record` stamps the golden with a `# context:` line (jobs, cpus,
load averages bracketing the run), and a diff that finds perf moves compares
the two contexts and says when one side started on a busy machine; only the
pre-run load counts, since the end sample includes the gate's own workers. The
unfiltered record is the scheduled baseline, not a loop check.

Exit codes — the gate's machine-readable verdict (every run can also write
a full verdict document with `--json PATH`):

    0  clean: no structural or perf moves
    1  moved: structural or perf moves, but no regression
    2  usage error: bad arguments, missing golden, no host target
    3  regressed: at least one fixture verdict is "regressed", or an
       authored exit or stdout oracle failed — including a `--record` that
       wrote the loss down (verdict "recorded_regression") and one that
       refused a contradicted oracle outright
    4  infrastructure failure: runner build failed, no binary produced,
       runner exited nonzero, or its records were unreadable

`--bisect` remaps for `git bisect run`, whose documented contract is: 125
skips the commit, 1..127 (except 125) mark it bad, 0 marks it good, and
any code above 127 aborts the bisect; so regressed -> 1 (bad), infrastructure
failure -> 125 (skip), everything else -> 0 (good), and usage errors ->
255 (abort, since a usage error says nothing about the commit).
"""

from __future__ import annotations

import argparse

import collections
import contextlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import sys
import threading

import corpus_records
from compiler_profile import CompilerProfileError, optimized_environment, record_cargo_build, require_optimized

try:
    import typesafe_key
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import typesafe_key

ROOT = Path(__file__).resolve().parent.parent
GOLDEN = ROOT / "tests" / "omega" / "corpus_outcomes.txt"

# Omega target names for hosts with a catalogued deployment profile.
HOST_TARGETS = {
    ("windows", "amd64"): "windows_x86_64",
    ("windows", "x86_64"): "windows_x86_64",
    ("linux", "x86_64"): "linux_x86_64",
    ("linux", "aarch64"): "linux_arm64",
    ("darwin", "arm64"): "macos_arm64",
    ("darwin", "x86_64"): "macos_x86_64",
}


def host_target() -> str | None:
    return HOST_TARGETS.get((platform.system().lower(), platform.machine().lower()))


NATIVE_EXIT_ORACLE = "expected_native_exit.txt"


def native_exit_oracle(fixture: Path, target: str) -> Path:
    """The fixture's authored exit promise for `target`.

    `expected_native_exit.<target>.txt` overrides the portable
    `expected_native_exit.txt`: a status's physical presentation belongs to
    the target contract (wiki/spec/language/process_exit.md), so one program
    may promise 212 on a Unix host for the semantic status -44 while another
    target keeps no promise at all.
    """
    specific = fixture / f"expected_native_exit.{target}.txt"
    return specific if specific.is_file() else fixture / NATIVE_EXIT_ORACLE


def oracle_failure(record: dict, detail: str, recordable: bool = None) -> dict:
    """One unmet authored promise, with whether `--record` may write the row.

    The two cases are different facts about the program, not two severities.
    When the observed status is `built` or `checked` the fixture produced a
    result and that result contradicts the author: the row is success-shaped,
    every reader that counts successes would count it, and writing it would
    pin the wrong runtime result as the expected one. When the status is a
    refusal, an incompleteness or a timeout the program produced no result at
    all, so today's row cannot be mistaken for the promise being met and
    writing it is the only way to stop the golden claiming an exit the
    fixture never reaches. Either way the gate prints the failure and exits
    `regressed`; recordability decides what the file says, never the verdict.
    """
    if recordable is None:
        recordable = record["status"] not in corpus_records.SUCCESS_STATUSES
    return {"fixture": record["fixture"], "detail": detail,
            "recordable": recordable}


def publication_observed(record: dict, native_leg: bool) -> bool:
    """Whether this record's leg published a native artifact for its fixture.

    The authored refusal oracle is a statement about native publication, so it
    is weighed only where publication happened. A native leg realizes every
    pass fixture; the check leg realizes exactly the pass and run fixtures
    whose own `build.omg` binds a program-entry root, and records that target
    as `check-target:<target>`. A leg that only accepted the source published
    nothing, so its `checked` row is not the refusal the oracle can expect, and
    weighing it there would invent a failure the leg never measured.
    """
    return native_leg or "check-target" in record["facts"]


def native_rejection_failures(records: list[dict], corpus: Path, target: str,
                              native_leg: bool = True) -> list[dict]:
    """Check build-only publication refusals without counting them as execution.

    Source acceptance and native publication have separate contracts. The
    gate derives this fact from the authored diagnostic fragments; a runner
    cannot assert it, and it cannot replace an execution promise. Weighing it
    wherever publication was observed is what lets one golden say a refusal was
    the authored one: the check leg publishes for entry-binding fixtures too,
    and a row there that names the target it realized but not the oracle it
    satisfied reports an authored refusal as a capability no reader can check.
    """
    failures = []
    for record in records:
        facts = record["facts"]
        facts.pop("native-rejection", None)
        fixture = corpus / record["fixture"]
        path = fixture / "expected_native_rejection.txt"
        if not path.exists() or not publication_observed(record, native_leg):
            continue
        try:
            fragments = [line.strip() for line in path.read_text(encoding="utf-8").splitlines()
                         if line.strip()]
            if not fragments:
                raise ValueError("no diagnostic fragments")
            if (record["tier"] != "pass" or fixture.name.endswith("_exit")
                    or native_exit_oracle(fixture, target).exists()
                    or any((fixture / f"expected_{stream}.txt").exists()
                           for stream in ("stdout", "stderr"))
                    or (fixture / "expected_terminal_targets.txt").exists()):
                raise ValueError("native rejection requires a build-only pass fixture without execution oracles")
        except (OSError, UnicodeError, ValueError) as error:
            failures.append(oracle_failure(
                record, f"{record['fixture']}: invalid native rejection expectation ({error})",
                recordable=False))
            continue
        diagnostics = "\n".join(record["diagnostics"])
        matched = (record["status"] == "rejected"
                   and not any(key in facts for key in ("exit", "signal", "stdout", "stderr"))
                   and all(fragment in diagnostics for fragment in fragments))
        facts["native-rejection"] = "match" if matched else "mismatch"
        record["expected_satisfied"] = matched
        if not matched:
            failures.append(oracle_failure(
                record, f"{record['fixture']}: expected native publication rejection containing "
                        f"{fragments!r}; observed {record['status']} {record['diagnostics']!r}",
                recordable=False))
    return failures


def terminal_target_failures(records: list[dict], corpus: Path) -> list[dict]:
    """One retained artifact must meet every authored target-installation row.

    This oracle runs on every corpus leg, independently of native execution or
    the selected interpreter engine. A checked/built row with missing or wrong
    target observations cannot be recorded as an expected success.
    """
    failures = []
    for record in records:
        path = corpus / record["fixture"] / "expected_terminal_targets.txt"
        if not path.exists():
            continue
        try:
            expectations = {}
            for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                words = line.split()
                if not words:
                    continue
                if len(words) != 2:
                    raise ValueError(f"line {line_number} needs target and decimal exit or trap")
                target, expected = words
                if (not target or not target.isascii()
                        or not all(character.isalnum() or character == "_" for character in target)
                        or target in expectations):
                    raise ValueError(f"line {line_number} has an invalid or duplicate target")
                if expected != "trap":
                    digits = expected.removeprefix("-")
                    if not digits or not digits.isascii() or not digits.isdecimal():
                        raise ValueError(f"line {line_number} needs decimal i32 exit or trap")
                    code = int(expected)
                    if not -(2 ** 31) <= code < 2 ** 31:
                        raise ValueError(f"line {line_number} exit is outside i32")
                    expected = str(code)
                expectations[target] = expected
            if not expectations:
                raise ValueError("no target expectations")
        except (OSError, UnicodeError, ValueError) as error:
            failures.append(oracle_failure(
                record, f"{record['fixture']}: invalid Terminal target expectation ({error})",
                recordable=False))
            continue
        facts = record.get("facts", {})
        mismatches = [f"{target} expected {expected}, observed {facts.get('terminal-target.' + target, 'missing')}"
                      for target, expected in expectations.items()
                      if facts.get("terminal-target." + target) != expected]
        if (record["status"] not in corpus_records.SUCCESS_STATUSES
                or facts.get("terminal-target-artifact") != "unchanged"
                or facts.get("terminal-targets") != "match" or mismatches):
            failures.append(oracle_failure(
                record, f"{record['fixture']}: expected unchanged Terminal artifact with exact target outcomes; "
                        f"observed {record['status']} {facts}; {'; '.join(mismatches)}"))
    return failures


def terminal_fence_failures(records: list[dict], corpus: Path) -> list[dict]:
    """Exact ordering events and pause progress are authored execution oracles."""
    failures = []
    kinds = {"lfence", "sfence", "mfence", "dmb_ld", "dmb_st", "dmb_sy"}
    for record in records:
        fixture = corpus / record["fixture"]
        path = fixture / "expected_terminal_fences.txt"
        if not path.exists():
            continue
        try:
            expectations = {}
            if not (fixture / "expected_terminal_targets.txt").is_file():
                raise ValueError("fence observations require explicit target installations")
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                words = line.split()
                if not words:
                    continue
                target, *sequence = words
                if (not target.isascii()
                        or not all(character.isalnum() or character == "_" for character in target)
                        or target in expectations or not sequence
                        or any(name not in kinds for name in sequence)):
                    raise ValueError(f"line {number} needs a unique target and known nonempty fence sequence")
                expectations[target] = ",".join(sequence)
            if not expectations:
                raise ValueError("no fence expectations")
        except (OSError, UnicodeError, ValueError) as error:
            failures.append(oracle_failure(
                record, f"{record['fixture']}: invalid Terminal fence expectation ({error})",
                recordable=False))
            continue
        facts = record.get("facts", {})
        mismatches = [f"{target} expected {expected}, observed {facts.get('terminal-fences.' + target, 'missing')}"
                      for target, expected in expectations.items()
                      if facts.get("terminal-fences." + target) != expected]
        if (record["status"] not in corpus_records.SUCCESS_STATUSES
                or facts.get("terminal-fence-artifact") != "unchanged"
                or facts.get("terminal-fences") != "match" or mismatches):
            failures.append(oracle_failure(
                record, f"{record['fixture']}: expected exact fence events with missing/wrong-model refusal "
                        f"and replay-free fuel progress; observed {record['status']} {facts}; {'; '.join(mismatches)}"))
    return failures


def entry_ram_failures(records: list[dict], corpus: Path, prefix: str) -> list[dict]:
    """An installed input promises observation of the same original backing."""
    failures = []
    for record in records:
        if not (corpus / record["fixture"] / "expected_entry_ram.txt").is_file():
            continue
        key = prefix + "entry-ram"
        if record["status"] not in corpus_records.SUCCESS_STATUSES or record.get("facts", {}).get(key) != "match":
            failures.append(oracle_failure(record,
                f"{record['fixture']}: expected exact {prefix}original entry RAM, observed {record['status']} {record.get('facts', {})}"))
    return failures


def native_exit_failures(records: list[dict], corpus: Path, target: str) -> list[dict]:
    """Check authored runtime oracles before comparing or recording observations.

    A golden may retain an unfinished compile, but cannot redefine an explicit
    runtime promise. `trap` means the ISA crash leaf: BRK/SIGTRAP on ARM64,
    UD2/SIGILL on Unix x86-64 (Linux and Intel macOS), or
    STATUS_ILLEGAL_INSTRUCTION on Windows.
    Segfaults, missing execution and timeouts are not successful traps.
    """
    traps = {"macos_arm64": {"exit": "signal", "signal": "5"},
             "linux_arm64": {"exit": "signal", "signal": "5"},
             "linux_x86_64": {"exit": "signal", "signal": "4"},
             "macos_x86_64": {"exit": "signal", "signal": "4"},
             "windows_x86_64": {"exit": str(-1073741795)}}
    failures = []
    for record in records:
        path = native_exit_oracle(corpus / record["fixture"], target)
        if not path.exists():
            continue
        try:
            expectation = path.read_text(encoding="utf-8").strip()
            if expectation == "trap":
                expected = traps[target]
            else:
                code = int(expectation)
                if not -(2 ** 31) <= code < 2 ** 31:
                    raise ValueError("exit code is outside i32")
                expected = {"exit": str(code)}
        except (OSError, UnicodeError, ValueError, KeyError) as error:
            # An unreadable promise says nothing about the program, so the
            # gate cannot write a row against it either.
            failures.append(oracle_failure(
                record,
                f"{record['fixture']}: invalid native exit expectation ({error})",
                recordable=False))
            continue
        if corpus_records.migration_gap_route(record) in ("compile", "native"):
            continue
        facts = record.get("facts", {})
        matches = all(facts.get(key) == value for key, value in expected.items())
        if record["status"] != "built" or not matches:
            failures.append(oracle_failure(
                record, f"{record['fixture']}: expected native {expectation}, "
                        f"observed {record['status']} {facts}"))
    return failures


def interpreter_exit_failures(records: list[dict], corpus: Path, target: str) -> list[dict]:
    """Preserve authored exit promises on interpreted execution too.

    Native/interpreter agreement does not prove the author's result. Unix
    presents the semantic i32 exit in its low byte; Windows presents it whole.
    """
    failures = []
    for record in records:
        path = native_exit_oracle(corpus / record["fixture"], target)
        if not path.is_file():
            continue
        if corpus_records.migration_gap_route(record) in ("compile", "interpreter"):
            continue
        expectation = "unreadable"
        try:
            expectation = path.read_text(encoding="utf-8").strip()
            facts = record["facts"]
            if expectation == "trap":
                # The portable engine reports a typed semantic trap. It is
                # not an OS child process and has no signal/NT status to map.
                # The present runner declines traps, so this observation is a
                # named producer dependency rather than synthetic agreement.
                matches = (facts.get("interpreter") == "trapped"
                           and "interpreter-exit" not in facts
                           and "interpreter-signal" not in facts)
            else:
                expected = int(expectation)
                if not -(2 ** 31) <= expected < 2 ** 31:
                    raise ValueError("exit code is outside i32")
                actual = int(facts.get("interpreter-exit", ""))
                if not -(2 ** 31) <= actual < 2 ** 31:
                    raise ValueError("interpreter exit is outside i32")
                presented = actual if target.startswith("windows_") else actual & 255
                matches = presented == expected
        except (OSError, UnicodeError, ValueError):
            matches = False
        accepted = (record["status"] in ("checked", "built")
                    or corpus_records.migration_gap_route(record) == "native")
        if not accepted or not matches:
            note = (" (requires an observed interpreter trap outcome)"
                    if expectation == "trap" else "")
            failures.append(oracle_failure(
                record,
                f"{record['fixture']}: expected interpreted {expectation}{note}, "
                f"observed {record['status']} {record['facts']}"))
    return failures


def execution_output_failures(records: list[dict], corpus: Path,
                             engine: str) -> list[dict]:
    """Enforce authored output for each requested execution engine.

    The runner compares output after normalizing line endings. Require its
    match fact rather than accepting an absent observation: unreadable oracle
    files, skipped execution and compile-only rows cannot establish a match.
    A timed-out process can print all expected bytes before hanging, so output
    also needs completed execution. This does not invent an expected exit code;
    only an authored exit/trap oracle constrains how the process terminates.
    """
    prefix = "interpreter-" if engine == "interpreter" else ""
    accepted_statuses = {"checked", "built"} if prefix else {"built"}
    failures = []
    for record in records:
        if corpus_records.migration_gap_route(record) in ("compile", engine):
            continue
        facts = record.get("facts", {})
        completed = (facts.get(prefix + "exit") not in (None, "timeout", "unrunnable")
                     or engine == "interpreter" and facts.get("interpreter") == "trapped"
                     and "interpreter-exit" not in facts
                     and "interpreter-signal" not in facts)
        accepted = (record["status"] in accepted_statuses or
                    engine == "interpreter" and
                    corpus_records.migration_gap_route(record) == "native")
        for stream in ("stdout", "stderr"):
            if not (corpus / record["fixture"] / f"expected_{stream}.txt").exists():
                continue
            if (not accepted or not completed
                    or facts.get(prefix + stream) != "match"):
                failures.append(oracle_failure(
                    record,
                    f"{record['fixture']}: expected {engine} {stream} match from completed "
                    f"execution, observed {record['status']} {facts}"))
    return failures


class InfrastructureFailure(Exception):
    """The gate could not measure at all — runner build failed, no binary
    was produced, the runner exited nonzero, or its records were
    unreadable. Distinct from a regression: exit 4, or 125 under --bisect
    so `git bisect run` skips the commit instead of blaming it."""


def windows_safe_target_dir(target_dir: Path) -> Path:
    """Keep Cargo target paths short enough on Windows that deep crate names
    (e.g. `build/resolved-layout-to-resolved-layout-.../out/lib....rlib`, ~135
    chars) stay under MSVC link.exe's 260-character MAX_PATH limit."""
    if os.name != "nt" or len(str(target_dir)) <= 100:
        return target_dir
    import hashlib
    digest = hashlib.sha256(
        str(target_dir.resolve()).encode("utf-8")).hexdigest()[:10]
    base = Path(os.environ.get("SYSTEMDRIVE", "C:") + "\\tmp")
    try:
        base.mkdir(parents=True, exist_ok=True)
    except OSError:
        base = Path(tempfile.gettempdir())
    return base / f"omega-tgt-{digest}"


def build_runner(root: Path = ROOT) -> Path:
    """Build `root`'s runner via cargo/mbx message-format JSON; return its path."""
    tool = "mbx" if shutil_which("mbx") else "cargo"
    args = [tool, "test", "--no-run", "--message-format=json",
            "-p", "compiler", "--test", "corpus_runner"]
    env = optimized_environment()
    if root != ROOT:
        # A compared checkout never shares this tree's target directory:
        # timestamp freshness could link the measured tree's artifacts.
        env["CARGO_TARGET_DIR"] = str(windows_safe_target_dir(root / "target"))
    else:
        configured = Path(env.get("CARGO_TARGET_DIR") or (root / "target"))
        safe_target = windows_safe_target_dir(configured)
        if safe_target != configured:
            env["CARGO_TARGET_DIR"] = str(safe_target)
    import build_slot
    with build_slot.hold(root):
        proc = subprocess.run(args, cwd=root, capture_output=True, text=True,
                              stdin=subprocess.DEVNULL, env=env)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout[-4000:])
        sys.stderr.write(proc.stderr[-4000:])
        raise InfrastructureFailure(
            f"corpus_gate: runner build failed ({proc.returncode})")
    executable = None
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "corpus_runner"
                and message.get("executable")):
            executable = message["executable"]
    if executable is None:
        raise InfrastructureFailure(
            "corpus_gate: build produced no corpus_runner binary")
    try:
        record_cargo_build(executable, proc.stdout)
    except CompilerProfileError as error:
        raise InfrastructureFailure(f"corpus_gate: {error}") from error
    return Path(executable)


def shutil_which(name: str):
    from shutil import which
    return which(name)


@contextlib.contextmanager
def windows_runner_job(process):
    """Tie a Windows runner tree to this gate's lifetime and cap its committed memory."""
    handle = getattr(process, "_handle", None)
    if os.name != "nt" or not isinstance(handle, int):
        yield
        return
    import ctypes
    kernel32 = ctypes.windll.kernel32
    job = kernel32.CreateJobObjectW(None, None)
    if not job:
        yield
        return
    try:
        class BasicLimits(ctypes.Structure):
            _fields_ = [("PerProcessUserTimeLimit", ctypes.c_int64),
                        ("PerJobUserTimeLimit", ctypes.c_int64),
                        ("LimitFlags", ctypes.c_uint32),
                        ("MinimumWorkingSetSize", ctypes.c_size_t),
                        ("MaximumWorkingSetSize", ctypes.c_size_t),
                        ("ActiveProcessLimit", ctypes.c_uint32),
                        ("Affinity", ctypes.c_size_t),
                        ("PriorityClass", ctypes.c_uint32),
                        ("SchedulingClass", ctypes.c_uint32)]

        class IoCounters(ctypes.Structure):
            _fields_ = [(name, ctypes.c_uint64) for name in (
                "ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
                "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]

        class ExtendedLimits(ctypes.Structure):
            _fields_ = [("BasicLimitInformation", BasicLimits),
                        ("IoInfo", IoCounters),
                        ("ProcessMemoryLimit", ctypes.c_size_t),
                        ("JobMemoryLimit", ctypes.c_size_t),
                        ("PeakProcessMemoryUsed", ctypes.c_size_t),
                        ("PeakJobMemoryUsed", ctypes.c_size_t)]

        limits = ExtendedLimits()
        flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        max_gb = int(os.environ.get("OMEGA_CORPUS_MAX_MEMORY_GB", "32") or "0")
        if max_gb > 0:
            flags |= 0x200  # JOB_OBJECT_LIMIT_JOB_MEMORY
            limits.JobMemoryLimit = max_gb * (1024 ** 3)
        limits.BasicLimitInformation.LimitFlags = flags
        if kernel32.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            kernel32.AssignProcessToJobObject(job, ctypes.c_void_p(handle))
        yield
    finally:
        kernel32.CloseHandle(job)


def run_runner(binary: Path, root: Path = ROOT, *, freshly_built=False) -> list[dict]:
    try:
        require_optimized(binary)
    except CompilerProfileError as error:
        raise InfrastructureFailure(f"corpus_gate: {error}") from error
    env = dict(os.environ)
    env.pop("NEXTEST", None)
    # A frozen runner may have been built in another checkout. Its fixture
    # root must agree with this gate's source/oracle tree, just as the SDK does.
    env["OMEGA_CORPUS_ROOT"] = str(root)
    # A reused build cache may retain another worktree's development library.
    # Corpus inputs belong to this checkout; preserve deliberate overrides.
    env.setdefault("OMEGA_LIBRARY_ROOT", str(root / "source/library"))
    # Records arrive in their own file: a fixture's build log is ordinary
    # compiler output on the runner's stdout and must not parse as a record.
    records_file = tempfile.NamedTemporaryFile(
        prefix="corpus-records-", suffix=".txt", delete=False)
    records_file.close()
    env["OMEGA_CORPUS_RECORDS"] = records_file.name
    try:
        proc = subprocess.Popen([str(binary)], cwd=root, text=True, env=env,
                                stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except OSError as error:
        # No runner started: do not leak its records file or blame a fixture.
        os.unlink(records_file.name)
        raise InfrastructureFailure(
            f"corpus_gate: cannot launch runner {binary}: {error}") from error
    buffered_stderr = collections.deque(maxlen=2000)
    root_reports = []
    root_prefix = "corpus_runner: fixture-root-v1 "

    def stream_progress():
        for line in proc.stderr:
            if line.startswith(root_prefix):
                root_reports.append(line[len(root_prefix):].strip())
            elif line.startswith("corpus_runner:"):
                sys.stderr.write(line)
                sys.stderr.flush()
            else:
                buffered_stderr.append(line)

    try:
        with windows_runner_job(proc):
            progress = threading.Thread(target=stream_progress, daemon=True)
            progress.start()
            stdout = proc.stdout.read()
            progress.join()
            proc.wait()
        records = Path(records_file.name).read_text(encoding="utf-8")
    finally:
        if os.path.exists(records_file.name):
            os.unlink(records_file.name)
    if proc.returncode != 0:
        sys.stderr.write(stdout[-2000:])
        sys.stderr.writelines(list(buffered_stderr)[-200:])
        raise InfrastructureFailure(
            f"corpus_gate: runner exited {proc.returncode}")
    # Freshly building a historical runner in root establishes its baked-in
    # default even before this protocol existed. A supplied binary has no such
    # guarantee: missing, ambiguous or mismatched reports cannot authorize rows.
    if root_reports or not freshly_built:
        if len(root_reports) != 1:
            raise InfrastructureFailure(
                "corpus_gate: runner must report exactly one fixture root; "
                "rebuild prebuilt runners that predate fixture-root-v1")
        try:
            reported_root = Path(os.fsdecode(bytes.fromhex(root_reports[0])))
        except ValueError as error:
            raise InfrastructureFailure("corpus_gate: unreadable runner fixture root") from error
        if not reported_root.is_absolute() or reported_root.resolve() != root.resolve():
            raise InfrastructureFailure(
                f"corpus_gate: runner fixture root {reported_root} differs from measured {root}")
    try:
        return corpus_records.parse(records)
    except ValueError as error:
        sys.stderr.write(records[-2000:])
        sys.stderr.writelines(list(buffered_stderr)[-200:])
        raise InfrastructureFailure(
            f"corpus_gate: runner emitted unreadable records: {error}")


# --- Jev advisory (--jev): per-diff classification + record verdict ---
# Worked example: build/experiments/failure-triage/CLASSIFY.md. Verdict
# reliability 4/4; per-diff labels inside a batch are contaminated by the
# worst member, so low verdicts re-ask each diff alone to localize.

def diff_questions(diffs: list) -> dict:
    questions = {}
    for index in range(len(diffs)):
        questions[f"diff_{index}"] = {
            "type": "choice",
            "instructions": (
                "Classify this fixture outcome change under the commit's "
                "subject and touched files. `intended`: what the commit was "
                "for, a harmless consequence, or only millis moved. "
                "`accidental`: status/expected_satisfied/diagnostics moved "
                "in a way this commit should not produce — pinning it would "
                "record a regression. `needs_review`: undecidable here."),
            "criteria": {
                "intended": "what this commit was for, harmless, or "
                            "timing-only",
                "accidental": "a semantic move this commit should not "
                              "produce — a regression",
                "needs_review": "cannot be decided from this evidence"}}
    questions["record_safe"] = {
        "type": "noul",
        "instructions": (
            "Would re-pinning the golden with these diffs record intended "
            "behavior only (high) or hide a regression (low)?"),
        "criteria": {
            "true": "every diff is intended or harmless; --record is safe",
            "false": "at least one diff looks accidental; --record would "
                     "pin a regression"}}
    return questions


def jev_advise(diffs: list) -> None:
    """Advisory annotation for structural diffs. Never raises; reports a
    missing key on stderr so an absent advisory is not read as a clean one;
    OMEGA_JEV_OFFLINE=1 suppresses."""
    if os.environ.get("OMEGA_JEV_OFFLINE", "").strip() == "1":
        print("  = jev advisory suppressed by OMEGA_JEV_OFFLINE=1",
              file=sys.stderr)
        return
    if not diffs:
        return
    key = typesafe_key.find_key(None)
    if not key:
        # A bare return here is indistinguishable from 'advisory ran and
        # found nothing wrong', which is how a missing key becomes a silent
        # blessing for --record. jev_attribute.py already names the missing
        # key on stderr; match it.
        print("  = jev advisory unavailable: no TYPESAFE_API_KEY "
              "(set it, or OMEGA_JEV_OFFLINE=1 to suppress) — no advisory "
              "was produced for this diff", file=sys.stderr)
        return
    try:
        subject = subprocess.run(
            ["git", "log", "-1", "--format=%s"], capture_output=True,
            text=True, timeout=10).stdout.strip()
        files = subprocess.run(
            ["git", "show", "--format=", "--name-only", "HEAD"],
            capture_output=True, text=True, timeout=10).stdout.split()[:40]
        state = {"commit_subject": subject,
                 "files_touched": files,
                 "fixture_diffs": diffs[:30]}
        request = {"model": typesafe_key.MODEL, "state": state,
                   "questions": diff_questions(diffs[:30])}
        response = typesafe_key.post(typesafe_key.ENDPOINT, key, request,
                                     timeout=30)
        if response is None:
            raise RuntimeError("no response")
        answers = response.get("answers", {})
        safe = answers.get("record_safe", {}).get("noul", 0.5)
        verdict = ("looks safe to --record" if safe >= 0.5
                   else "--record would likely pin a regression")
        print(f"  = jev advisory: record_safe {safe:.2f} — {verdict}")
        flagged = [i for i in range(len(diffs[:30]))
                   if answers.get(f"diff_{i}", {}).get("choice")
                   == "accidental"]
        if safe < 0.5 and flagged:
            # Batch labels are contaminated by the worst member, and a solo
            # intended/accidental re-ask conflates "unexplained by this
            # commit" with "intended". Ask the causal question instead:
            # could THIS commit plausibly produce this diff?
            for index in flagged[:10]:
                solo = {"model": typesafe_key.MODEL,
                        "state": {"commit_subject": subject,
                                  "files_touched": files,
                                  "fixture_diffs": [diffs[index]]},
                        "questions": {"caused_by_commit": {
                            "type": "noul",
                            "instructions": (
                                "Could this commit plausibly produce this "
                                "fixture outcome change? Timing-only "
                                "movement is noise (high). A status or "
                                "diagnostic change outside the commit's "
                                "domain is unexplained — unexplained means "
                                "suspicious (low)."),
                            "criteria": {
                                "true": "this commit could plausibly cause "
                                        "this outcome change",
                                "false": "unexplained by this commit — "
                                         "suspicious"}}}}
                try:
                    one = typesafe_key.post(typesafe_key.ENDPOINT, key, solo,
                                            timeout=30)
                    if one is None:
                        raise RuntimeError("no response")
                    caused = (one.get("answers", {})
                              .get("caused_by_commit", {}).get("noul", 0.5))
                except Exception:
                    caused = 0.5
                if caused < 0.5:
                    print(f"    SUSPECT: {diffs[index]['fixture']} "
                          f"(unexplained by commit, {caused:.2f})")
                else:
                    print(f"    probably fine: {diffs[index]['fixture']} "
                          f"({caused:.2f})")
        elif flagged:
            for index in flagged[:10]:
                print(f"    possibly accidental: "
                      f"{diffs[index]['fixture']}")
    except Exception as error:
        print(f"  = jev advisory unavailable: {type(error).__name__}",
              file=sys.stderr)


# Status ranks for the pass/run tiers: built means the artifact exists,
# checked means compilation succeeded, anything else is a non-movement
# floor. Fail tiers invert the goal: refusing is success there.
STATUS_RANK = {"built": 3, "checked": 2}

# A fail fixture is pinned when the compiler refuses it: a rejection, or an
# Incomplete coverage refusal for a body this compiler cannot lower yet. Its
# expected fragment, checked separately, names which refusal it pins.
REFUSALS = ("rejected", "incomplete")


def classify(before: dict | None, after: dict | None) -> tuple[str, list[str]]:
    """One fixture's verdict: (verdict, reasons).

    Verdicts: clean, moved, improved, regressed, perf_slower, perf_faster,
    new, removed. Regressed dominates improved dominates moved; the reasons
    list collects everything that fired. Diagnostics compare normalized
    (corpus_records.normalized_diagnostics) so a diagnostic relabel is not
    a move; recorded_at and regressed_from are provenance the gate derives,
    never observations, so neither is compared.
    """
    if before is None:
        return "new", ["fixture added"]
    if after is None:
        return "removed", ["fixture removed"]
    regressed, improved, moved = [], [], []
    tier = after["tier"]
    old_status, new_status = before["status"], after["status"]
    if corpus_records.native_rejection_satisfied(after):
        if not corpus_records.native_rejection_satisfied(before):
            improved.append("authored native publication rejection now satisfied")
    elif corpus_records.native_rejection_satisfied(before):
        regressed.append("authored native publication rejection no longer satisfied")
    elif tier == "fail":
        # Refusal is the goal here; a fail fixture that stopped refusing is a
        # soundness regression, never an improvement.
        if old_status in REFUSALS and new_status not in REFUSALS:
            regressed.append("fail fixture stopped rejecting")
        elif old_status not in REFUSALS and new_status in REFUSALS:
            improved.append(f"status {old_status} -> {new_status}")
        elif old_status != new_status:
            moved.append(f"status {old_status} -> {new_status}")
    else:
        old_rank = STATUS_RANK.get(old_status, 0)
        new_rank = STATUS_RANK.get(new_status, 0)
        if new_rank < old_rank:
            regressed.append(f"status {old_status} -> {new_status}")
        elif new_rank > old_rank:
            improved.append(f"status {old_status} -> {new_status}")
    if before["expected_satisfied"] is True and after["expected_satisfied"] is False:
        regressed.append("rejected without its expected fragment")
    elif before["expected_satisfied"] is False and after["expected_satisfied"] is True:
        improved.append("expected fragment now satisfied")
    old_facts, new_facts = before["facts"], after["facts"]
    old_stdout, new_stdout = old_facts.get("stdout"), new_facts.get("stdout")
    if old_stdout == "match" and new_stdout != "match":
        regressed.append("stdout match -> " + (new_stdout or "absent"))
    elif old_stdout == "differs" and new_stdout == "match":
        improved.append("stdout differs -> match")
    if old_facts.get("exit") != new_facts.get("exit"):
        moved.append(f"exit {old_facts.get('exit')} -> {new_facts.get('exit')}")
    elif old_facts != new_facts:
        moved.append("facts moved")
    if (corpus_records.normalized_diagnostics(before["diagnostics"])
            != corpus_records.normalized_diagnostics(after["diagnostics"])):
        moved.append("diagnostics moved")
    if regressed:
        return "regressed", regressed + improved + moved
    if improved:
        return "improved", improved + moved
    if moved:
        return "moved", moved
    # Perf moves only apply to otherwise-clean fixtures: a structural move
    # already explains the fixture and the timing is noise on top.
    old_ms, new_ms = before.get("millis", 0), after.get("millis", 0)
    if old_ms >= 2000 and new_ms >= 2 * old_ms and new_ms - old_ms >= 5000:
        return "perf_slower", [f"{old_ms}ms -> {new_ms}ms (>=2x slowdown)"]
    if old_ms >= 2000 and new_ms <= old_ms // 2 and old_ms - new_ms >= 5000:
        return "perf_faster", [f"{old_ms}ms -> {new_ms}ms (>=2x faster)"]
    return "clean", []


def move_classes(diffs: list) -> str:
    """Separate RECORDING DEBT from behaviour that moved.

    A fixture added, removed or repaired by another lane prints as a move
    for every developer until its golden row catches up, and that debt
    accumulates: it is most of what an unfiltered run shows on a clean
    checkout. Naming the classes lets a reader see in one line whether the
    run is telling them something about their own change. Classes derive
    from classify() verdicts so this line cannot disagree with the exit
    code — the old hand-rolled mapping called a fail fixture that stopped
    rejecting "improved", which pinned a soundness regression as progress.
    """
    labels = {"new": "fixture added, golden not recorded",
              "removed": "fixture removed, golden row left behind",
              "improved": "improved, golden not re-recorded",
              "regressed": "REGRESSED",
              "moved": "diagnostics or facts moved",
              "perf_slower": "diagnostics or facts moved",
              "perf_faster": "diagnostics or facts moved"}
    classes = collections.Counter(labels[diff["verdict"]] for diff in diffs)
    if not classes:
        return "corpus_gate: no structured diffs to classify"
    parts = ", ".join(f"{count} {name}"
                      for name, count in classes.most_common())
    return f"corpus_gate: of those — {parts}"


def shown(value) -> str:
    """One field of a record, as a diff line shows it."""
    if isinstance(value, list):
        return " | ".join(value) if value else "(no diagnostics)"
    if isinstance(value, dict):
        return " ".join(f"{key}:{item}" for key, item in value.items()) or "(no facts)"
    return str(value)


def baseline_path(revision: str, leg: str = "") -> Path:
    """The cached outcomes of `revision`, one file per commit and leg.

    The checked-in golden pins outcomes for `main` as a whole, so it disagrees
    with any checkout whose base carries another lane's in-flight movement, and
    a single run against it cannot separate that lane's drift from this one's.
    `--compare` measures the base commit itself and keeps those rows here, so
    each later measurement against the same base costs one corpus pass.
    """
    suffix = f"-{leg}" if leg else ""
    return ROOT / "build" / "corpus_baselines" / f"{revision}{suffix}.txt"


COMPARE_WORKTREE = ROOT / "build" / "cmp"
FILTER_VARIABLE = "OMEGA_CORPUS_FIXTURE_FILTER"


def checkout_base(revision: str) -> Path:
    """A detached worktree at `revision` with its own build directory, so the
    base runner can never link the measured tree's edits or artifacts."""
    if (COMPARE_WORKTREE / ".git").exists():
        moved = git_out("-C", str(COMPARE_WORKTREE), "checkout", "-q", "-f",
                        "--detach", revision)
    else:
        git_out("worktree", "prune")
        moved = git_out("worktree", "add", "-q", "--detach",
                        str(COMPARE_WORKTREE), revision)
    if moved is None:
        raise InfrastructureFailure(
            f"corpus_gate: cannot check out {revision} at {COMPARE_WORKTREE}")
    return COMPARE_WORKTREE


def base_records_for(revision: str, leg: str, selected: list, baseline_runner: Path = None) -> list:
    """`revision`'s rows for the selected fixtures, measuring only those the
    cache lacks. A fixture the base does not have stays absent: it is new."""
    cache = baseline_path(revision, leg)
    cached = ({record["fixture"]: record for record in corpus_records.read(cache)}
              if cache.is_file() else {})
    missing = [fixture for fixture in selected if fixture not in cached]
    if missing:
        worktree = checkout_base(revision)
        # A warm cache contains every old fixture but cannot contain rows for
        # newly added programs. Do not launch a base runner with an empty
        # effective selection: absence on the base is a comparison fact, not
        # a runner failure. Fixture discovery uses these same main.omg roots.
        missing = [fixture for fixture in missing
                   if (worktree / "tests/omega" / fixture / "main.omg").is_file()]
    if missing:
        selection = os.environ.get(FILTER_VARIABLE, "")
        exact = ",".join(missing)
        # Exact names keep a top-up cheap; the original selection already
        # covers a cold cache, and an over-long list would exceed the
        # environment block on Windows.
        if len(missing) < len(selected) and len(exact) < 20_000:
            selection = exact
        previous = os.environ.get(FILTER_VARIABLE)
        if selection:
            os.environ[FILTER_VARIABLE] = selection
        try:
            if baseline_runner is not None:
                if git_out("-C", str(worktree), "rev-parse", "HEAD") != revision:
                    raise InfrastructureFailure("corpus_gate: frozen baseline runner needs the exact compared source checkout")
                measured = run_runner(baseline_runner, worktree, freshly_built=False)
            else:
                measured = run_runner(build_runner(worktree), worktree, freshly_built=True)
        finally:
            if previous is None:
                os.environ.pop(FILTER_VARIABLE, None)
            else:
                os.environ[FILTER_VARIABLE] = previous
        for record in measured:
            record["recorded_at"] = revision[:12]
            cached[record["fixture"]] = record
        cache.parent.mkdir(parents=True, exist_ok=True)
        cache.write_text(corpus_records.render(
            [cached[fixture] for fixture in sorted(cached)]), encoding="utf-8")
    return [cached[fixture] for fixture in selected if fixture in cached]


def compare_with_revision(options, leg, revision: str, head_records: list) -> int:
    """Diff this tree's run against `revision`'s, fixture by fixture."""
    base = base_records_for(revision, leg, [record["fixture"] for record in head_records],
                            getattr(options, "baseline_runner", None))
    result = corpus_records.compare_records(base, head_records)
    for line in result["losses"]:
        print(f"- {line}")
    for line in result["gains"]:
        print(f"+ {line}")
    dirty = bool(dirty_paths())
    print(f"corpus_gate: compared {result['fixtures']} fixtures against "
          f"{revision[:12]}{' (head has uncommitted edits)' if dirty else ''}: "
          f"{len(result['losses'])} lost, {len(result['gains'])} gained")
    write_verdict(options, {
        "schema": "omega-corpus-compare-v1",
        "base": revision,
        "head": git_out("rev-parse", "HEAD"),
        "dirty": dirty,
        "leg": leg or "checked",
        **result,
    })
    return 1 if result["losses"] else 0


def git_out(*arguments, timeout=None) -> str | None:
    try:
        result = subprocess.run(["git", *arguments], cwd=ROOT,
                                capture_output=True, text=True,
                                env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
                                timeout=timeout)
    except subprocess.TimeoutExpired:
        print("corpus_gate: optional Git provenance lookup timed out; "
              "revision distance unavailable", file=sys.stderr)
        return None
    except OSError:
        # Provenance is optional metadata, including when a test or caller
        # supplies an isolated environment without the host's Git executable.
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def commits_between(revision):
    """Optional history distance; unavailable history must not delay verdicts."""
    count = git_out("rev-list", "--count", f"{revision}..HEAD", timeout=2)
    return int(count) if count and count.isdigit() else None


def load_average():
    """1-minute load average, or None where the host has none (Windows)."""
    try:
        return round(os.getloadavg()[0], 2)
    except (AttributeError, OSError):
        return None


def measurement_context(load_start) -> dict:
    """The conditions a corpus run measured under. `jobs` is the worker
    override the runner read (None = runner default). `load1_start` is the
    external load before the gate started; `load1_end` includes the gate's
    own workers (the runner defaults to up to 12), so it is informational
    only and never judges contention. Written into the golden as a
    `# context:` comment by --record, and compared against the golden's own
    context on every diff."""
    return {"schema": "omega-corpus-context-v1",
            "jobs": os.environ.get("OMEGA_CORPUS_JOBS") or None,
            "cpus": os.cpu_count(),
            "platform": platform.system().lower(),
            "machine": platform.machine().lower(),
            "load1_start": load_start,
            "load1_end": load_average()}


def contended(context) -> bool:
    """True when the machine was already busy before the gate started: the
    pre-run load met or exceeded half the hardware's parallelism, so the
    runner's workers competed with outside work and millis inflated for
    reasons unrelated to the compiler. Only `load1_start` counts — the end
    sample carries the gate's own workers, and a 15-fixture run on an idle
    12-cpu host was measured moving it from 5.19 to 8.43; a full run
    saturates it by itself."""
    if not context:
        return False
    cpus = context.get("cpus") or 0
    load = context.get("load1_start")
    return (cpus > 0 and load is not None
            and load >= cpus * CONTENDED_LOAD_FRACTION)


# The runner defaults to one worker per cpu (capped at 12), so any outside
# load competes with it; half the machine already busy is where the
# resulting millis inflation stops being plausibly small. A judgment
# threshold, not a measured one — it only chooses when to print a warning.
CONTENDED_LOAD_FRACTION = 0.5


def perf_context_warnings(golden_context, current_context) -> list[str]:
    """Why this run's perf moves may be measurement noise. Perf moves are
    the only verdict drawn from millis, and millis answer the machine as
    much as the compiler — the warnings name the specific mismatch so the
    reader knows which direction the noise pushed."""
    warnings = []
    if golden_context is None:
        warnings.append(
            "golden predates context stamping — perf moves cannot be "
            "checked against its recording conditions; re-record on a "
            "quiet machine to make them meaningful")
        return warnings
    golden_jobs = golden_context.get("jobs")
    current_jobs = current_context.get("jobs") if current_context else None
    if (golden_jobs is not None and current_jobs is not None
            and str(golden_jobs) != str(current_jobs)):
        warnings.append(
            f"worker counts differ (golden jobs={golden_jobs}, this run "
            f"jobs={current_jobs}) — record and diff at the same "
            "OMEGA_CORPUS_JOBS before trusting perf moves")
    if contended(golden_context):
        warnings.append(
            f"the golden was recorded on a busy machine (pre-run load "
            f"{golden_context.get('load1_start')} on "
            f"{golden_context.get('cpus')} cpus); fixtures only look faster "
            "now — treat perf_faster moves as noise")
    if contended(current_context):
        warnings.append(
            f"this run started on a busy machine (pre-run load "
            f"{current_context.get('load1_start')} on "
            f"{current_context.get('cpus')} cpus); fixtures only look "
            "slower — treat perf_slower moves as noise")
    return warnings


def jobs_line(current_context, golden_context) -> str:
    """The worker count this run measured under, beside the golden's own.

    Fixture outcomes were measured to depend on `OMEGA_CORPUS_JOBS` in some
    selections — a pass fixture that checked alone at jobs=1 was rejected by
    the same revision, tree and binary in a 39-fixture selection at jobs=3 —
    so a corpus number without its worker count is not comparable to its
    neighbours. Printed on every verdict, not only when the two counts differ:
    a difference is already a warning, and the value is what a reader needs
    when nothing warns at all.
    """
    current = (current_context or {}).get("jobs") or "runner default"
    line = f"corpus_gate: measured at jobs={current}"
    if not golden_context:
        return (line + " (the golden carries no context line, so its worker "
                       "count is unknown)")
    golden_jobs = golden_context.get("jobs") or "runner default"
    return line + f", golden jobs={golden_jobs}"


def porcelain_path(line: str) -> str:
    """The path one `git status --porcelain` line names.

    `git status --porcelain` writes `XY path` per line, but git_out strips the
    whole buffer, so a first line like ` M x` arrives as `M x`; parse the path
    after the status token instead of a fixed column.
    """
    return line.strip().split(None, 1)[-1].split(" -> ")[-1].strip().strip('"')


def dirty_paths() -> list:
    """Every path git reports as differing from HEAD, untracked files included.

    `--untracked-files=no` was the original spelling here, and it is why a
    newly authored fixture was invisible to the stamp: a directory of untracked
    `main.omg` files left the tree reading "clean", so `--record` stamped rows
    for a revision in which the fixture did not exist. A new fixture is exactly
    what untracked files are, so the dirty test must see them.
    """
    status = git_out("status", "--porcelain", "--untracked-files=all") or ""
    return [porcelain_path(line) for line in status.splitlines() if line]


def measurement_stamp(golden) -> str | None:
    """First 12 hex of HEAD, plus `+wt` when files other than the golden
    differ from HEAD — including untracked ones — the row's provenance for
    `--record`. None without git."""
    revision = git_out("rev-parse", "HEAD")
    if not revision:
        return None
    # Record targets exempt from provenance dirt: every corpus golden, since a
    # multi-golden --record dirties the earlier goldens while measuring the next.
    record_targets = {
        path.resolve().relative_to(ROOT).as_posix()
        for path in (ROOT / "tests" / "omega").glob("corpus_*.txt")
    }
    dirty = any(path not in record_targets for path in dirty_paths())
    return revision[:12] + ("+wt" if dirty else "")


def upstream_main() -> str | None:
    """The published main ref this checkout can see; None when it can see none.

    Only remote main refs count as published ancestors. A feature branch's
    configured upstream is not published main. This is a local read, no fetch.
    """
    candidates = []
    for line in (git_out("for-each-ref", "--format=%(refname)",
                         "refs/remotes/") or "").splitlines():
        if line.endswith("/main"):
            candidates.append(line)
    for candidate in candidates:
        if git_out("rev-parse", "--verify", "--quiet", candidate):
            return candidate
    return None


def retain_record_source(revision: str) -> str | None:
    """Retain exact source independently of main/queue SHA and branch cleanup.

    The bundle excludes only a locally known published ancestor. It can be
    restored in a clone containing that prerequisite; with no main it contains
    the complete history. Never replace a previously retained source bundle.
    """
    ref = "refs/corpus-observations/" + revision
    directory = ROOT / "build" / "corpus-source"
    bundle = directory / (revision + ".bundle")
    main = upstream_main()
    base = git_out("merge-base", revision, main) if main else None
    if base == revision:
        return None  # Already retained in published history; no export needed.
    temporary = None
    try:
        directory.mkdir(parents=True, exist_ok=True)
        def run(*args):
            return subprocess.run(["git", *args], cwd=ROOT, capture_output=True,
                                  text=True, timeout=60, check=True)
        run("update-ref", ref, revision)
        if not bundle.exists():
            # NamedTemporaryFile's open handle prevents Git writing on Windows.
            with tempfile.NamedTemporaryFile(dir=directory, suffix=".bundle", delete=False) as stream:
                temporary = Path(stream.name)
            args = ["bundle", "create", str(temporary), ref]
            if base:
                args.append("^" + base)
            run(*args)
            run("bundle", "verify", str(temporary))
            # Concurrent writers may publish the same immutable source; no
            # existing evidence is overwritten.
            try:
                os.link(temporary, bundle)
            except FileExistsError:
                pass
        # verify/list-heads check only the header and prerequisites. Unbundle
        # also authenticates the full pack (including its checksum), so a
        # truncated/corrupt export cannot be recorded as retained provenance.
        # It imports objects only; it moves no branch, checkout or source.
        heads = run("bundle", "unbundle", str(bundle)).stdout.splitlines()
        if f"{revision} {ref}" not in heads:
            return "retained source bundle does not bind the stamped revision"
        print(f"corpus_gate: retained source evidence {bundle}; publish with PR evidence",
              file=sys.stderr)
        return None
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        return f"cannot retain stamped source evidence: {error}"
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def revision_absent_paths(revision: str, paths) -> list:
    """Which of `paths` the revision's tree does not contain, in one Git read.

    `git cat-file --batch-check` answers one `<revision>:<path>` query per
    input line, so a whole record's fixtures cost one process rather than one
    each. A query Git cannot answer counts as absent: this is a claim a reader
    has to be able to check, and an unanswerable read has not shown it.
    """
    paths = list(paths)
    if not paths:
        return []
    queries = "".join(f"{revision}:{path}\n" for path in paths)
    try:
        result = subprocess.run(["git", "cat-file", "--batch-check"], cwd=ROOT,
                                input=queries, capture_output=True, text=True,
                                env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
                                timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return paths
    answers = result.stdout.splitlines() if result.returncode == 0 else []
    if len(answers) != len(paths):
        return paths
    return [path for path, answer in zip(paths, answers)
            if answer.endswith(" missing")]


def stamp_provenance_problem(stamp: str | None, fixtures=()) -> str | None:
    """Why `--record` must not write rows stamped `stamp`, or None when it may.

    A source commit need not be on main. Preserve it in an exportable bundle
    before writing evidence so a squash or branch cleanup cannot erase it.
    Main/queue SHA movement never calls for another measurement.

    `fixtures` is what the run would stamp. A stamp with no `+wt` marker
    claims the named revision had the tree these rows were measured from, so
    each fixture must exist there for a reader to check the row at all. A row
    whose fixture is absent from the revision it names — the shape a stamp
    written over untracked new fixtures produced — is unresolvable evidence
    and is refused. A `+wt` stamp names the base a dirty tree sat on instead,
    so absence there is expected: the marker is what tells the reader the
    fixture came from the working tree, not from that revision.
    """
    if not stamp:
        return None
    revision = stamp[: -len("+wt")] if stamp.endswith("+wt") else stamp
    full_revision = git_out("rev-parse", "--verify", "--quiet", f"{revision}^{{commit}}")
    if not full_revision:
        return (f"the revision it names ({revision}) does not resolve to a commit "
                f"in this checkout")
    problem = retain_record_source(full_revision)
    if problem:
        return problem
    if stamp.endswith("+wt"):
        return None
    corpus = ROOT / "tests" / "omega"
    queries, outside = [], []
    for fixture in sorted(set(fixtures)):
        try:
            queries.append((corpus / fixture).relative_to(ROOT).as_posix())
        except ValueError:
            # A fixture id that does not name a path under the corpus root was
            # not discovered by walking it, so nothing at `revision` holds it.
            outside.append(fixture)
    absent = outside + revision_absent_paths(revision, queries)
    if not absent:
        return None
    named = ", ".join(absent[:3]) + (f" and {len(absent) - 3} more"
                                     if len(absent) > 3 else "")
    return (f"{len(absent)} of the fixtures it would stamp do not exist at "
            f"{revision}: {named}")


def golden_label(golden: Path) -> str:
    """How a message names a golden: checkout-relative where it is inside."""
    try:
        return golden.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return str(golden)


def verdict_document(options, leg, target, verdict, exit_code, records,
                     classified, oracle_failures, infrastructure_error,
                     golden_provenance=None, perf_context=None):
    """The `--json` verdict document; written on every outcome so a caller
    (or `git bisect run` wrapper) never has to grep the text summary."""
    golden = Path(options.golden)
    golden_path = golden_label(golden)
    # The one measurement LOST cannot make: a row can keep its key and lose the
    # reason stored under it. Read from the file so the number is the golden's
    # own state, not this run's subset, and reported on every verdict.
    reason_gaps = corpus_records.reason_free_gap(golden)
    by_status = collections.Counter(record["status"] for record in records)
    by_verdict = collections.Counter(entry["verdict"] for entry in classified)
    # Named, not just counted: a caller reading the document needs to know
    # WHICH fixture stopped pinning its reason to do anything about it.
    wrong_reason = [record["fixture"] for record in records
                    if record["expected_satisfied"] is False]
    fail_not_rejected = [
        record["fixture"] for record in records
        if record["tier"] == "fail" and record["status"] == "checked"]
    lost_successes = [record["fixture"]
                      for record in corpus_records.lost_successes(records)]
    off_target = corpus_records.pinned_off_target(records)
    subset = (bool(options.filter) or bool(options.shard)
              or bool(os.environ.get("OMEGA_CORPUS_FIXTURE_FILTER", "").strip())
              or bool(os.environ.get("OMEGA_CORPUS_SHARD", "").strip()))
    return {
        "schema": "omega-corpus-verdict-v1",
        "revision": git_out("rev-parse", "HEAD"),
        "dirty": bool(dirty_paths()),
        "leg": leg or "checked",
        "target": target,
        "golden": golden_path,
        "golden_reason_gaps": reason_gaps,
        "subset": subset,
        "filter": options.filter,
        "shard": options.shard,
        "verdict": verdict,
        "exit_code": exit_code,
        "counts": {
            "fixtures": len(records),
            "by_status": dict(by_status),
            "wrong_reason": len(wrong_reason),
            "wrong_reason_fixtures": wrong_reason,
            "fail_not_rejected": len(fail_not_rejected),
            "fail_not_rejected_fixtures": fail_not_rejected,
            "lost_successes": len(lost_successes),
            "lost_success_fixtures": lost_successes,
            "pinned_off_target": len(off_target),
            "pinned_off_target_fixtures": [record["fixture"]
                                           for record in off_target],
            "by_verdict": dict(by_verdict)},
        "oracle_failures": oracle_failures,
        "infrastructure_error": infrastructure_error,
        "golden_provenance": golden_provenance,
        "perf_context": perf_context,
        "observations": records,
        "fixtures": classified}


def write_verdict(options, document):
    if not options.json:
        return
    path = Path(options.json)
    if path.parent != Path(""):
        path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")


def pinned_regression_lines(records, list_rows=False) -> list[str]:
    """Golden rows pinned in a status their tier contradicts.

    `regressed-from` counts only rows that recorded a success before; a
    pass fixture pinned `rejected` from birth is still a pinned failure,
    so this count covers every off-target pin, token or not. One summary
    line when the flag is absent — the list is there, not the noise."""
    off_target = corpus_records.pinned_off_target(records)
    if not off_target:
        return []
    refused = [record for record in off_target
               if record["tier"] in ("pass", "run")]
    succeeded = [record for record in off_target
                 if record["tier"] == "fail"]
    carried = sum(1 for record in off_target
                  if record.get("regressed_from"))
    lines = [f"corpus_gate: {len(refused)} pass/run row(s) pinned without "
             f"a success and {len(succeeded)} fail row(s) pinned as a "
             f"success ({carried} carry regressed-from)"]
    if not list_rows:
        lines.append("corpus_gate:   list them with "
                     "--list-pinned-regressions")
        return lines
    for record in off_target:
        line = f"corpus_gate:   {record['fixture']} {record['status']}"
        if record.get("regressed_from"):
            line += (" (lost " + corpus_records.format_regression(
                record["regressed_from"])[
                    len(corpus_records.REGRESSION_PREFIX):] + ")")
        lines.append(line)
    return lines


def list_pinned_regressions(golden: Path) -> int:
    """Inspect recorded debt without claiming a new compiler observation.

    This roster must remain usable before a compiler build is available. Keep
    it ahead of runner preparation, migration admission and measurement so an
    inspection cannot execute fixtures or rewrite their recorded outcomes.
    """
    try:
        records = corpus_records.read(golden)
    except (OSError, UnicodeError, ValueError) as error:
        print(f"corpus_gate: cannot read golden {golden}: {error}", file=sys.stderr)
        return 4
    print(f"corpus_gate: recorded outcomes from {golden}; no fixtures executed")
    lines = pinned_regression_lines(records, list_rows=True)
    for line in lines:
        print(line)
    if not lines:
        print("corpus_gate: no pinned regressions in the recorded outcomes")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--record", action="store_true",
                        help="rewrite the golden from this run instead of diffing")
    parser.add_argument("--jev", action="store_true",
                        help="annotate structural diffs with an advisory "
                             "intended/accidental classification + "
                             "record_safe verdict (needs TYPESAFE_API_KEY)")
    parser.add_argument("--runner", type=Path,
                        help="use an existing corpus_runner binary instead of building")
    parser.add_argument("--baseline-runner", type=Path,
                        help="with --compare, use an explicitly frozen optimized baseline runner; "
                             "its exact compared source worktree and fixture-root report remain required")
    parser.add_argument("--golden", type=Path,
                        help="read/write this golden instead of the selected leg's default")
    parser.add_argument("--native", action="store_true",
                        help="build pass and run fixtures for this host, execute "
                             "the run tier, *_exit and explicit native-exit fixtures, "
                             "and diff against "
                             "tests/omega/corpus_native_<target>.txt")
    parser.add_argument("--native-target", metavar="TARGET",
                        help="implies --native: build for TARGET instead of the "
                             "host (macos_x86_64 executes under Rosetta on an "
                             "Apple-silicon host) and diff against "
                             "tests/omega/corpus_native_<TARGET>.txt")
    parser.add_argument("--interpret", action="store_true",
                        help="also execute runnable fixtures through the shared "
                             "Terminal program/instance core; default golden "
                             "tests/omega/corpus_interpreted.txt must exist, "
                             "or supply --golden")
    parser.add_argument("--checked-interpreter", action="store_true",
                        help="implies --interpret: explicitly use the independent "
                             "checked-tree engine instead of Terminal (never a "
                             "fallback); default golden "
                             "tests/omega/corpus_checked_interpreted.txt must exist, "
                             "or supply --golden")
    parser.add_argument("--migration-roster", type=Path, metavar="PATH",
                        help="reviewed capability/task/route canary designations; "
                             "exact pending roots join the runner; "
                             "explicit unimplemented debt never counts as success")
    parser.add_argument("--both", action="store_true",
                        help="run native and selected interpreter legs together, "
                             "record whether they agree, and diff against "
                             "tests/omega/corpus_both_<target>.txt "
                             "(supply --golden when absent)")
    parser.add_argument("--compare", metavar="REVISION",
                        help="diff this tree's run against REVISION's, built "
                             "in its own worktree (build/cmp) and cached per "
                             "commit under build/corpus_baselines; exits 1 "
                             "when any fixture loses an outcome, expected "
                             "fragment or execution fact")
    parser.add_argument("--filter",
                        help="comma-separated fixture fragments — run and diff "
                             "only matching fixtures (sets "
                             "OMEGA_CORPUS_FIXTURE_FILTER for the runner)")
    parser.add_argument("--shard",
                        help="k/N — deterministic hash-slice of the corpus "
                             "(sets OMEGA_CORPUS_SHARD for the runner)")
    parser.add_argument("--list-pinned-regressions", action="store_true",
                        help="read the selected golden and name rows pinned in a "
                             "status their tier contradicts; does not build or run "
                             "fixtures; accepts only golden/target selection options")
    parser.add_argument("--write-moves", type=Path, metavar="PATH",
                        help="write every moved fixture's BEFORE and AFTER "
                             "records to PATH as JSON, untruncated — the "
                             "summary below cuts each field at 300 columns, "
                             "which hides the diagnostic that actually moved")
    parser.add_argument("--bisect", action="store_true",
                        help="exit for `git bisect run`: 1 on regressed "
                             "(bad), 125 on infrastructure failure (skip), "
                             "0 otherwise (good), 255 on usage error (abort)")
    parser.add_argument("--json", metavar="PATH",
                        help="write the machine-readable verdict document "
                             "to PATH on every outcome, including "
                             "infrastructure failure and --record refusal")
    try:
        options = parser.parse_args()
    except SystemExit as error:
        if not error.code:
            return 0
        # Under `git bisect run` every code in 1..127 (except 125) means
        # "bad", so a usage error would convict an innocent commit.
        return 255 if "--bisect" in sys.argv else error.code
    if options.list_pinned_regressions:
        selection_options = {"list_pinned_regressions", "golden", "native",
                             "native_target", "interpret", "checked_interpreter",
                             "both"}
        incompatible = ["--" + name.replace("_", "-")
                        for name, value in vars(options).items()
                        if value is not None and value is not False
                        and name not in selection_options]
        if incompatible:
            print("corpus_gate: --list-pinned-regressions is read-only; "
                  "cannot combine with " + ", ".join(incompatible), file=sys.stderr)
            return 255 if options.bisect else 2
    use_default_golden = options.golden is None
    if use_default_golden:
        options.golden = GOLDEN
    leg = ""
    target = None
    # `git bisect run` reads 1..127 (except 125) as "bad"; usage errors say
    # nothing about the commit under test, so they abort instead.
    usage_error = 255 if options.bisect else 2
    if options.both:
        options.native = True
        options.interpret = True
    if options.checked_interpreter:
        options.interpret = True
        os.environ["OMEGA_CORPUS_CHECKED_INTERPRETER"] = "1"
    else:
        os.environ.pop("OMEGA_CORPUS_CHECKED_INTERPRETER", None)
    if options.native_target:
        options.native = True
        os.environ["OMEGA_CORPUS_NATIVE_TARGET"] = options.native_target
    if options.interpret:
        os.environ["OMEGA_CORPUS_INTERPRET"] = "1"
        leg = "checked-interpreted" if options.checked_interpreter else "interpreted"
        if use_default_golden and not options.native:
            name = "checked_interpreted" if options.checked_interpreter else "interpreted"
            options.golden = ROOT / "tests" / "omega" / f"corpus_{name}.txt"
    if options.native:
        target = options.native_target or host_target()
        if target is None:
            print("corpus_gate: this host has no catalogued Omega target for --native")
            return usage_error
        os.environ["OMEGA_CORPUS_NATIVE"] = "1"
        leg = ("checked-both" if options.checked_interpreter else "both") if options.interpret else "native"
        if use_default_golden:
            name = ("checked_both" if options.checked_interpreter else "both") if options.interpret else "native"
            options.golden = ROOT / "tests" / "omega" / f"corpus_{name}_{target}.txt"
    if options.list_pinned_regressions:
        return list_pinned_regressions(options.golden)
    compare_revision = None
    if options.baseline_runner and not options.compare:
        print("corpus_gate: --baseline-runner requires --compare")
        return usage_error
    if options.compare:
        if options.record:
            print("corpus_gate: --compare measures a base; it does not record a golden")
            return usage_error
        if not (options.filter or options.shard
                or os.environ.get("OMEGA_CORPUS_FIXTURE_FILTER", "").strip()
                or os.environ.get("OMEGA_CORPUS_SHARD", "").strip()):
            # Both sides of an unfiltered comparison run the whole corpus,
            # about two hours here: scheduled workload, not a loop check.
            print("corpus_gate: --compare runs the selection twice; name the "
                  "fixtures the change can move with --filter <group>/,... "
                  "(or sample with --shard k/N)")
            return usage_error
        compare_revision = git_out("rev-parse", "--verify", f"{options.compare}^{{commit}}")
        if not compare_revision:
            print(f"corpus_gate: --compare names no commit: {options.compare}")
            return usage_error
        # A foreign native target keeps its own cache beside the host's.
        compare_leg = f"{leg}-{options.native_target}" if options.native_target else leg

    subset = (bool(options.filter) or bool(options.shard)
              or bool(os.environ.get("OMEGA_CORPUS_FIXTURE_FILTER", "").strip())
              or bool(os.environ.get("OMEGA_CORPUS_SHARD", "").strip()))
    if options.filter:
        os.environ["OMEGA_CORPUS_FIXTURE_FILTER"] = options.filter
    if options.shard:
        os.environ["OMEGA_CORPUS_SHARD"] = options.shard
    # Prior timings widen per-fixture caps for known-slow fixtures
    # (adaptive max(cap, 3x golden millis) — the runner derives them).
    if options.golden.is_file():
        os.environ["OMEGA_CORPUS_TIMINGS"] = str(options.golden)
    # Resolve the reviewed roster before launching the runner. Only its exact
    # pending roots join collection; no canary is copied into a passing tier and
    # the runner still produces the ordinary exit/stdout/diagnostic observations.
    try:
        roster = corpus_records.migration_roster(
            options.migration_roster, ROOT / "tests" / "omega")
    except (OSError, ValueError, TypeError) as error:
        print(f"corpus_gate: invalid migration roster: {error}", file=sys.stderr)
        code = 125 if options.bisect else 4
        write_verdict(options, verdict_document(
            options, leg, target, "infrastructure_failed", code, [], [],
            [], str(error)))
        return code
    os.environ["OMEGA_CORPUS_MIGRATION_FIXTURES"] = "\n".join(sorted({
        fixture for fixture, _route in roster if fixture.startswith("pending/")}))
    # Sampled before and after the run so the recorded context brackets the
    # measurement; millis answer the machine as much as the compiler.
    load_start = load_average()
    try:
        binary = options.runner or build_runner()
        records = run_runner(binary, freshly_built=options.runner is None)
        context = measurement_context(load_start)
        if compare_revision is not None:
            return compare_with_revision(options, compare_leg, compare_revision, records)
    except InfrastructureFailure as error:
        print(str(error), file=sys.stderr)
        code = 125 if options.bisect else 4
        write_verdict(options, verdict_document(
            options, leg, target, "infrastructure_failed", code, [], [],
            [], str(error)))
        return code
    try:
        missing = corpus_records.missing_migration_canaries(
            records, roster, os.environ.get("OMEGA_CORPUS_FIXTURE_FILTER", ""),
            os.environ.get("OMEGA_CORPUS_SHARD", ""))
        if missing:
            raise ValueError("selected migration canaries absent from runner: "
                             + ", ".join(missing)
                             + "; runner did not exercise the reviewed selection")
        corpus_records.classify_migration(records, roster)
    except (OSError, ValueError, TypeError) as error:
        print(f"corpus_gate: invalid migration evidence: {error}", file=sys.stderr)
        code = 125 if options.bisect else 4
        write_verdict(options, verdict_document(
            options, leg, target, "infrastructure_failed", code, records, [],
            [], str(error)))
        return code
    gaps = [record for record in records
            if corpus_records.migration_gap_route(record) is not None]
    for record in gaps:
        facts = record["facts"]
        print(f"corpus_gate: {record['fixture']} unimplemented "
              f"{facts['migration-capability']} ({facts['migration-task']}, "
              f"{facts['migration-route']})")
    failures = terminal_target_failures(records, ROOT / "tests" / "omega")
    failures.extend(terminal_fence_failures(records, ROOT / "tests" / "omega"))
    # A publication refusal is weighed on every leg that published, not on the
    # native leg alone: the check leg realizes an entry-binding pass fixture
    # itself, so its own row is the evidence the authored oracle speaks about.
    failures.extend(native_rejection_failures(
        records, ROOT / "tests" / "omega", target or host_target() or "unknown",
        native_leg=options.native))
    if options.native:
        failures.extend(native_exit_failures(records, ROOT / "tests" / "omega", target))
        failures.extend(entry_ram_failures(records, ROOT / "tests" / "omega", ""))
        failures.extend(execution_output_failures(
            records, ROOT / "tests" / "omega", "native"))
    if options.interpret:
        failures.extend(entry_ram_failures(records, ROOT / "tests" / "omega", "interpreter-"))
        failures.extend(interpreter_exit_failures(
            records, ROOT / "tests" / "omega", target or host_target() or "unknown"))
        failures.extend(execution_output_failures(
            records, ROOT / "tests" / "omega", "interpreter"))
    # One read of the golden serves both the regression provenance below and
    # the diff further down. An unfiltered --record can replace an unreadable
    # golden; a subset cannot preserve the pins it did not measure.
    prior_records, golden_error = None, None
    if options.golden.is_file():
        try:
            prior_records = corpus_records.read(options.golden)
        except (OSError, UnicodeError, ValueError) as error:
            golden_error = error
    if options.record and subset and golden_error is not None:
        message = (f"cannot preserve unmeasured fixtures in unreadable golden "
                   f"{options.golden}: {golden_error}")
        print(f"corpus_gate: --record refused: {message}", file=sys.stderr)
        write_verdict(options, verdict_document(
            options, leg, target, "infrastructure_failed", usage_error, records,
            [], [failure["detail"] for failure in failures], message))
        return usage_error
    # Report the golden's reason-free rows before any verdict is drawn from it.
    # This is a read-only count: the rows are left byte-for-byte as they are,
    # because the state is evidence of how the record was damaged and a repair
    # would destroy it. `LOST` cannot report this — the key is present — so the
    # operator has to be told here, before trusting a reason-level verdict.
    if prior_records:
        reason_gaps = corpus_records.reason_free_rows(prior_records)
        if reason_gaps:
            print(f"corpus_gate: {golden_label(options.golden)} carries "
                  f"{len(reason_gaps)} of {len(prior_records)} rows asserting "
                  f"an expected/unexpected outcome with no stored diagnostic "
                  f"line — their outcome is an observation, their reason is "
                  f"unsupported, and LOST cannot see them because the key is "
                  f"present. Reason-level verdicts drawn from them are "
                  f"unsupported; --record refuses to write new rows of that "
                  f"shape")
    prior = {record["fixture"]: record for record in (prior_records or [])}

    # Each row this run replaces carries forward the last success it recorded
    # when today's observation is not a success. A reader then sees the
    # difference between a fixture that has never built on this host and one
    # that built and no longer does, and the token disappears by itself when
    # the fixture builds again.
    for record in records:
        record["regressed_from"] = corpus_records.carried_regression(
            prior.get(record["fixture"]), record)

    oracle_failures = [failure["detail"] for failure in failures]
    for detail in oracle_failures:
        print(f"corpus_gate: {detail}", file=sys.stderr)
    # An unmet promise the fixture never produced is writable; one it
    # contradicted is not. Recording the first stops the golden asserting a
    # result the program does not reach, and does not make the run clean:
    # the verdict stays `regressed` either way.
    contradicted = [failure for failure in failures if not failure["recordable"]]
    if failures and options.record and contradicted:
        print("corpus_gate: --record refused: "
              + ", ".join(sorted({failure["fixture"]
                                  for failure in contradicted}))
              + " produced a result contradicting an authored oracle; "
                "recording would pin it as the expected one",
              file=sys.stderr)
        code = 1 if options.bisect else 3
        write_verdict(options, verdict_document(
            options, leg, target, "regressed", code, records, [],
            oracle_failures, None))
        return code
    # An unmet oracle without --record still stops the gate, but only once
    # the diff below is computed and written: one fixture's unmet target
    # promise, possibly inherited from main, must not hide what this run
    # moved everywhere else, or `--write-moves` has nothing to attribute.
    # --record stamps each row measured this run with its provenance; rows
    # merged forward in a subset record keep the stamp they were born with.
    stamp = measurement_stamp(options.golden) if options.record else None
    if stamp:
        problem = stamp_provenance_problem(
            stamp, [record["fixture"] for record in records])
        if problem:
            message = (
                f"a row stamped @{stamp} would claim an observation at a revision "
                f"cannot be retained or inspected: {problem}. Preserve the exact "
                f"source evidence and retry recording on this fixed candidate; "
                f"do not move HEAD to main or relabel the observation")
            print(f"corpus_gate: --record refused: {message}", file=sys.stderr)
            write_verdict(options, verdict_document(
                options, leg, target, "infrastructure_failed", usage_error, records,
                [], oracle_failures, message))
            return usage_error
        for record in records:
            record["recorded_at"] = stamp
    # The second measurement LOST cannot make. A row that carries an
    # expected/unexpected token and no tab line asserts that the fixture's
    # authored fragment was satisfied and stores no reason it was: the key is
    # present, so nothing reads as lost. corpus_runner cannot write that state
    # — `expected_fragment_satisfied` answers None, no token, when a fixture
    # produced no diagnostics, and every diagnostic it did produce becomes a
    # tab line — so a run that holds one has been spliced, scripted or merged,
    # and the rows this run is writing are refused. Rows already in the golden
    # are read and reported by the load below, never repaired: silently
    # rewriting them would destroy the evidence of how the record was damaged.
    reason_free = corpus_records.reason_free_rows(records)
    if options.record and reason_free:
        named = ", ".join(entry["fixture"] for entry in reason_free[:3])
        if len(reason_free) > 3:
            named += f" and {len(reason_free) - 3} more"
        message = (
            f"{len(reason_free)} of the rows this run would write carry an "
            f"expected/unexpected token with no diagnostic line ({named}). The "
            f"runner cannot produce that state — a token is written only when "
            f"the fixture produced diagnostics, and each becomes a tab line — "
            f"so the row would assert a satisfied fragment with no stored "
            f"reason, which LOST cannot see because the key is present. Re-run "
            f"--record from the runner's own output rather than a splice or an "
            f"edited records file")
        print(f"corpus_gate: --record refused: {message}", file=sys.stderr)
        write_verdict(options, verdict_document(
            options, leg, target, "infrastructure_failed", usage_error, records,
            [], oracle_failures, message))
        return usage_error
    rendered = corpus_records.render(records, context)

    if options.record:
        merged = records
        # A subset record merges into the golden: fixtures that ran get their
        # fresh records, everything else keeps its pin and its text. A subset
        # writes no context line; each row's @stamp says when it was measured.
        # Only an unfiltered record re-renders the file with this run's
        # context.
        if subset and prior_records is not None:
            carried = {record["fixture"]: record for record in prior_records}
            carried.update({record["fixture"]: record for record in records})
            merged = [carried[fixture] for fixture in sorted(carried)]
            # Splice the measured rows into the existing text so a subset
            # record leaves every other row, comment and context line
            # byte-for-byte; concurrent subset records then rebase cleanly.
            rendered = corpus_records.splice(
                options.golden.read_text(encoding="utf-8"), records)
        elif subset:
            print(f"corpus_gate: warning — no golden at {options.golden}; "
                  f"recording only the {len(records)} selected fixtures",
                  file=sys.stderr)
        # `Path.write_text` grew `newline` in 3.10; this host runs 3.9, and the
        # golden's identity is byte-sensitive, so open the stream directly
        # rather than letting the platform translate the line endings.
        with options.golden.open("w", encoding="utf-8", newline="\n") as stream:
            stream.write(rendered)
        recorded_gaps = [record for record in merged
                         if record["status"] == "unimplemented"
                         or record["facts"].get("interpreter") == "unimplemented"]
        lost = corpus_records.lost_successes(merged)
        print(f"corpus_gate: recorded {len(records)} fixture outcomes "
              f"to {options.golden}; {len(recorded_gaps)} unimplemented rows "
              f"remain, {len(lost)} rows record a lost success")
        print(jobs_line(context, corpus_records.read_context(options.golden)))
        for line in pinned_regression_lines(merged):
            print(line)
        if failures:
            # The golden now says what the fixture does; it does not say that
            # is acceptable. A recorded regression is still a regression.
            for record in corpus_records.lost_successes(records):
                print(f"corpus_gate: {record['fixture']} recorded as "
                      f"{record['status']}, lost "
                      + corpus_records.format_regression(
                          record["regressed_from"])[
                              len(corpus_records.REGRESSION_PREFIX):],
                      file=sys.stderr)
            code = 1 if options.bisect else 3
            write_verdict(options, verdict_document(
                options, leg, target, "recorded_regression", code, records,
                [], oracle_failures, None))
            return code
        write_verdict(options, verdict_document(
            options, leg, target, "recorded_unimplemented" if recorded_gaps else "recorded",
            1 if recorded_gaps else 0, records, [], [], None))
        return 1 if recorded_gaps else 0

    if not options.golden.is_file():
        print(f"corpus_gate: no golden at {options.golden}; "
              f"run --record first", file=sys.stderr)
        return usage_error
    if golden_error is not None:
        print(f"corpus_gate: unreadable golden {options.golden}: "
              f"{golden_error}", file=sys.stderr)
        return usage_error
    golden_context = corpus_records.read_context(options.golden)
    golden = dict(prior)
    actual = {record["fixture"]: record for record in records}

    structural_moves = []
    structured_diffs = []
    classified = []
    perf_moves = []
    perf_notes = []
    for fixture, record in actual.items():
        before = golden.get(fixture)
        verdict, reasons = classify(before, record)
        if verdict != "clean":
            classified.append({"fixture": fixture, "verdict": verdict,
                               "reasons": reasons,
                               "before": before, "after": record})
        if before is None:
            structural_moves.append(f"+ {fixture}: new fixture "
                                  f"({record['status']})")
            structured_diffs.append(
                {"fixture": fixture, "verdict": verdict,
                 "before": {"status": "absent"},
                 "after": {"status": record["status"],
                           "expected_satisfied": record["expected_satisfied"],
                           "diagnostics": record["diagnostics"]}})
            continue
        if verdict == "perf_slower":
            perf_moves.append(f"~ {fixture}: {reasons[0]}")
            continue
        if verdict == "perf_faster":
            perf_notes.append(f"  {fixture}: {reasons[0]}")
            continue
        if verdict == "clean":
            continue
        # Show the first field that moved; diagnostics compare normalized
        # (a relabel is not a move) but print their raw text.
        fields = {
            "status": (before["status"], record["status"]),
            "expected_satisfied": (before["expected_satisfied"],
                                   record["expected_satisfied"]),
            "facts": (before["facts"], record["facts"]),
            "diagnostics": (corpus_records.normalized_diagnostics(
                                before["diagnostics"]),
                            corpus_records.normalized_diagnostics(
                                record["diagnostics"]))}
        for field, (old, new) in fields.items():
            if old != new:
                structural_moves.append(
                    f"~ {fixture}: {field} moved\n"
                    f"    - {shown(before[field])[:300]}\n"
                    f"    + {shown(record[field])[:300]}")
                structured_diffs.append(
                    {"fixture": fixture, "verdict": verdict,
                     "before": {"status": before["status"],
                                "expected_satisfied":
                                    before["expected_satisfied"],
                                "diagnostics": before["diagnostics"]},
                     "after": {"status": record["status"],
                               "expected_satisfied":
                                   record["expected_satisfied"],
                               "diagnostics": record["diagnostics"]}})
                break
    if not subset:
        for fixture in golden:
            if fixture not in actual:
                verdict, reasons = classify(golden[fixture], None)
                classified.append({"fixture": fixture, "verdict": verdict,
                                   "reasons": reasons,
                                   "before": golden[fixture], "after": None})
                structural_moves.append(f"- {fixture}: removed from corpus")
                structured_diffs.append(
                    {"fixture": fixture, "verdict": verdict,
                     "before": {"status": golden[fixture]["status"]},
                     "after": {"status": "absent"}})

    counts = {}
    for record in records:
        counts[record["status"]] = counts.get(record["status"], 0) + 1
    # Two different conditions, reported apart. A fail fixture that rejected
    # without its fragment rejected for the WRONG reason; one that checked did
    # not reject at all, which for the fail tier usually means its expectation
    # names a stage this targetless route never reaches.
    # Both conditions name their fixtures. A count alone is unactionable: a
    # reader cannot tell which fixture stopped pinning its reason, and the
    # number sat at 2 across unrelated sessions because nobody could.
    wrong_reason_fixtures = [r["fixture"] for r in records
                             if r["expected_satisfied"] is False]
    not_rejected_fixtures = [
        r["fixture"] for r in records
        if r["tier"] == "fail" and r["status"] == "checked"
    ]
    summary = (f"{len(records)} fixtures ({counts}), "
               f"{len(wrong_reason_fixtures)} rejected without their expected "
               f"fragment, "
               f"{len(not_rejected_fixtures)} fail fixtures did not reject")
    executed = [r for r in records if "exit" in r.get("facts", {})]
    if executed:
        exits = collections.Counter(r["facts"]["exit"] for r in executed)
        differs = sum(1 for r in executed if r["facts"].get("stdout") == "differs")
        summary += (f"; executed {len(executed)} (exit codes {dict(exits)}), "
                    f"{differs} with stdout differing from expected")
    verdict_counts = collections.Counter(entry["verdict"]
                                         for entry in classified)
    if verdict_counts:
        summary += f"; verdicts {dict(verdict_counts)}"
    # Golden rows can outlive the commit that measured them when a subset
    # --record merges old rows forward; provenance is the staleness readout
    # for the fixtures this run actually exercised.
    golden_provenance = corpus_records.provenance(
        [golden[fixture] for fixture in actual if fixture in golden],
        commits_between)
    provenance = ("corpus_gate: "
                  + corpus_records.provenance_line(golden_provenance))
    # Every reason-bearing verdict carries the worker count it was measured
    # at, because fixture outcomes themselves were measured to move with it.
    jobs = jobs_line(context, golden_context)
    # Perf moves are the only verdict drawn from millis; when the machines
    # or worker counts behind the two measurements disagree, say so instead
    # of letting a busy box read as a compiler slowdown.
    perf_warnings = (perf_context_warnings(golden_context, context)
                     if perf_moves or perf_notes else [])
    perf_context = {"golden": golden_context, "current": context,
                    "warnings": perf_warnings}

    def print_expectation_defects() -> None:
        """Name the fixtures behind the two expectation counts.

        These are not moves -- both sides of a comparison can carry the same
        defect, so a `clean` run reports them too. A fail fixture that rejects
        without its fragment rejects for the WRONG reason and no longer pins
        what its directory claims; one that checked did not reject at all.
        """
        for fixture in wrong_reason_fixtures[:40]:
            print(f"corpus_gate: {fixture} rejected without its expected "
                  f"fragment — it no longer pins its authored reason")
        if len(wrong_reason_fixtures) > 40:
            print(f"corpus_gate: … {len(wrong_reason_fixtures) - 40} more "
                  f"rejecting without their expected fragment")
        for fixture in not_rejected_fixtures[:40]:
            print(f"corpus_gate: {fixture} did not reject at all")
        if len(not_rejected_fixtures) > 40:
            print(f"corpus_gate: … {len(not_rejected_fixtures) - 40} more "
                  f"fail fixtures that did not reject")

    def write_moves() -> None:
        if not options.write_moves:
            return
        options.write_moves.parent.mkdir(parents=True, exist_ok=True)
        options.write_moves.write_text(
            json.dumps(structured_diffs, indent=2) + "\n", encoding="utf-8")
        print(f"corpus_gate: wrote {len(structured_diffs)} moved fixture(s) "
              f"to {options.write_moves}")

    if failures and not options.record:
        for line in structural_moves[:60]:
            print(line)
        if len(structural_moves) > 60:
            print(f"… {len(structural_moves) - 60} more moves")
        print(f"corpus_gate: {len(oracle_failures)} unmet oracle(s), "
              f"{len(structural_moves)} structural move(s) — {summary}")
        print(provenance)
        print(jobs)
        write_moves()
        code = 1 if options.bisect else 3
        write_verdict(options, verdict_document(
            options, leg, target, "regressed", code, records, classified,
            oracle_failures, None, golden_provenance, perf_context))
        return code
    if gaps and not structural_moves and not perf_moves:
        print(f"corpus_gate: coverage incomplete — {len(gaps)} unimplemented canaries")
        write_moves()
        code = 1
        write_verdict(options, verdict_document(
            options, leg, target, "unimplemented", code, records, classified,
            [], None, golden_provenance, perf_context))
        return code
    if not structural_moves and not perf_moves:
        print(f"corpus_gate: clean — {summary}")
        print(provenance)
        print(jobs)
        print_expectation_defects()
        for line in pinned_regression_lines(prior_records):
            print(line)
        for line in perf_notes[:20]:
            print(line)
        for warning in perf_warnings:
            print(f"corpus_gate: perf context — {warning}")
        # A clean run still rewrites the file, so a reader never mistakes a
        # previous run's moves for this one's.
        write_moves()
        write_verdict(options, verdict_document(
            options, leg, target, "clean", 0, records, classified, [], None,
            golden_provenance, perf_context))
        return 0
    for line in structural_moves[:60]:
        print(line)
    for line in perf_moves[:60]:
        print(line)
    remaining = len(structural_moves) + len(perf_moves) - 120
    if remaining > 0:
        print(f"… {remaining} more moves")
    for line in perf_notes[:20]:
        print(line)
    for warning in perf_warnings:
        print(f"corpus_gate: perf context — {warning}")
    regressed = verdict_counts.get("regressed", 0) > 0
    verdict = "regressed" if regressed else "moved"
    code = (1 if regressed else 0) if options.bisect else \
        (3 if regressed else 1)
    print(f"corpus_gate: {len(structural_moves)} structural move(s), "
          f"{len(perf_moves)} perf move(s) — {summary}")
    print(move_classes(structured_diffs))
    print(provenance)
    print(jobs)
    print_expectation_defects()
    for line in pinned_regression_lines(prior_records):
        print(line)
    write_moves()
    if options.jev:
        jev_advise(structured_diffs)
    write_verdict(options, verdict_document(
        options, leg, target, verdict, code, records, classified, [], None,
        golden_provenance, perf_context))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
