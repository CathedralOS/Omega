"""Compare the names Cargo's test harnesses list with the results the readiness run actually saw."""
from collections import Counter
from pathlib import Path
import re
import subprocess
from time import monotonic


TEST_RESULT = re.compile(r"^test (.+?) \.\.\. (ok|FAILED|ignored)(?:,.*)?$")
TARGET = re.compile(r"^\s*Running (.+?) \(.+\)$")
DOC = re.compile(r"^\s*Doc-tests \S+$")
TOTAL = re.compile(r"^(\d+) tests?, (\d+) benchmarks?$")


def target_header(line):
    if match := TARGET.fullmatch(line):
        return match[1].replace("\\", "/")
    return "doctests" if DOC.fullmatch(line) else None


OS_REASON = r"[^\n]+ \(os error \d+\)"
CARGO_LAUNCH = re.compile(
    r"error: (?:doc)?test failed, to rerun pass `[^`\n]+`\n\nCaused by:\n"
    r"  could not execute process `[^\n]+` \(never executed\)\n\nCaused by:\n"
    rf"  (?P<reason>{OS_REASON})"
)


def launch_failures(log, action, exit_code):
    """Extract confirmed Cargo/rustdoc launch diagnostics from one command's log."""
    if not exit_code:
        return []
    lines = log.splitlines()
    rows, failed_names = [], set()
    target = section = None
    for i, line in enumerate(lines):
        if section is None:
            if header := target_header(line):
                target = header
                # Cargo couldn't start the harness: no libtest output intervenes.
                if cause := CARGO_LAUNCH.fullmatch("\n".join(lines[i + 1:i + 8])):
                    rows.append(dict(action=action, target=target, reason=cause["reason"]))
            elif target and re.fullmatch(r"running \d+ tests?", line):
                section = "results"
                failed_names.clear()
        elif line.startswith("test result: "):
            target = section = None
        elif target == "doctests":
            if line in ("successes:", "failures:"):
                section = line
            elif section == "results" and (result := re.fullmatch(r"test (.+) \.\.\. FAILED", line)):
                failed_names.add(result[1])
            elif section == "failures:" and (block := re.fullmatch(r"---- (.+) stdout ----", line)):
                # Rustdoc emits this diagnostic instead of running the example.
                if (block[1] in failed_names and i + 3 < len(lines) and not lines[i + 2]
                        and (lines[i + 3] == "failures:" or lines[i + 3].startswith("---- "))):
                    if cause := re.fullmatch(rf"Couldn't run the test: ({OS_REASON})", lines[i + 1]):
                        rows.append(dict(action=action, target=target, test=block[1], reason=cause[1]))
    return rows


def abnormal_exits(log, exit_code):
    """Attribute Cargo's process-exit diagnostic to its harness even after a passing libtest summary."""
    if not exit_code:
        return []
    lines = log.splitlines()
    target, capture = None, False
    issues = []
    for i, line in enumerate(lines):
        if line.startswith("test result: "):
            capture = False
        elif line in ("successes:", "failures:"):
            capture = True
        elif not capture and (header := target_header(line)):
            target = header
        elif target and not capture and re.fullmatch(r"error: (?:doc)?test failed, to rerun pass `[^`]+`", line):
            if lines[i + 1:i + 3] == ["", "Caused by:"] and i + 3 < len(lines):
                termination = re.fullmatch(r"  process didn't exit successfully: `.+` \((.+)\)", lines[i + 3])
                if termination:
                    issues.append(f"{target}: harness exited abnormally ({termination[1]})")
            target = None
    return issues


def list_command(command):
    # Keep Cargo's target selection and libtest's exact/skip/ignored filters.
    separator = command.index("--")
    args = [arg for arg in command[separator + 1:] if arg != "--show-output"]
    return [*command[:separator + 1], "--list", *args]


def listed_tests(log):
    targets, names, issues = set(), Counter(), []
    target = None
    count = None
    for line in log.splitlines():
        if header := target_header(line):
            if target is not None and count is not None:
                issues.append(f"{target}: missing list total")
            if header in targets:
                issues.append(f"{header}: repeated target")
            target, count = header, Counter()
            targets.add(header)
        elif target is not None and (match := TOTAL.fullmatch(line)):
            if count is None or (count["test"], count["benchmark"]) != tuple(map(int, match.groups())):
                issues.append(f"{target}: named entries disagree with {line!r}")
            count = None
        elif target is not None and (match := re.fullmatch(r"(.+): (test|benchmark)", line)):
            if count is None:
                issues.append(f"{target}: entry after list total")
                continue
            count[match[2]] += 1
            names[target, match[1]] += 1
    if target is not None and count is not None:
        issues.append(f"{target}: missing list total")
    if not targets:
        issues.append("no listed test targets")
    return targets, names, issues


def compare(listing, execution, tests):
    targets, listed, issues = listed_tests(listing)
    ran_targets = {header for line in execution.splitlines() if (header := target_header(line))}
    for target in sorted(targets - ran_targets):
        issues.append(f"{target}: listed target absent from execution")
    for target in sorted(ran_targets - targets):
        issues.append(f"{target}: executed target absent from listing")
    ran = Counter()
    for row in tests:
        name = re.sub(r" - (should panic|compile fail|compile)$", "", row["test"])
        key = row["target"].replace("\\", "/"), name
        ran[key] += 1
        if row["status"] == "ignored":
            issues.append(f"{key[0]}::{name}: ignored")
    for key in sorted(listed.keys() | ran.keys()):
        if listed[key] != 1 or ran[key] != 1:
            issues.append(f"{key[0]}::{key[1]}: listed {listed[key]}, reported {ran[key]}")
    return {"listed": sum(listed.values()), "reported": sum(ran.values()), "targets": len(targets), "issues": issues}


def verify_suite(report, stage, directory, environment):
    suite = report["suites"][stage]
    command = list_command(suite["command"])
    path = directory / f"{stage}-list.log"
    started = monotonic()
    try:
        with path.open("wb") as stream:
            code = subprocess.run(command, env=environment, stdin=subprocess.DEVNULL,
                                  stdout=stream, stderr=subprocess.STDOUT).returncode
    except OSError as error:
        path.write_text(str(error), encoding="utf-8")
        code = 1
    execution = directory / suite.get("log", f"{stage}.log")
    listing = path.read_text(encoding="utf-8", errors="replace")
    run = execution.read_text(encoding="utf-8", errors="replace")
    suite["launch_failures"] = launch_failures(run, "test", suite["exit"]) + launch_failures(listing, "list", code)
    roster = compare(listing, run, suite["tests"])
    roster["issues"].extend(abnormal_exits(run, suite["exit"]))
    roster["issues"].extend("listing " + issue for issue in abnormal_exits(listing, code))
    if code:
        roster["issues"].append(f"list command exited {code}")
    roster.update(command=command, seconds=round(monotonic() - started, 2), log=path.name, exit=code)
    suite["roster"] = roster
    report["measurement_errors"].extend(f"{stage} roster: {issue}" for issue in roster["issues"])
