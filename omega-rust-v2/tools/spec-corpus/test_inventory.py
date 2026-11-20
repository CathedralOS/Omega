"""Compare the names Cargo's test harnesses list with the results the readiness run actually saw."""
from collections import Counter
from pathlib import Path
import re
import subprocess
from time import monotonic


TARGET = re.compile(r"^\s*Running (.+?) \(.+\)$")
DOC = re.compile(r"^\s*Doc-tests \S+$")
TOTAL = re.compile(r"^(\d+) tests?, (\d+) benchmarks?$")


def target_header(line):
    if match := TARGET.fullmatch(line):
        return match[1].replace("\\", "/")
    return "doctests" if DOC.fullmatch(line) else None


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
    roster = compare(path.read_text(encoding="utf-8", errors="replace"),
                     execution.read_text(encoding="utf-8", errors="replace"), suite["tests"])
    if code:
        roster["issues"].append(f"list command exited {code}")
    roster.update(command=command, seconds=round(monotonic() - started, 2), log=path.name, exit=code)
    suite["roster"] = roster
    report["measurement_errors"].extend(f"{stage} roster: {issue}" for issue in roster["issues"])
