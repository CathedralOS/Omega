"""Compare L4's owned preparation reasons with typed refusals observed by passing tests.

The denominator is `PreparationReason::ALL`, printed by one trusted L4 test and
cross-checked against the `preparation_reasons!` declaration. A reason counts only
when a passing L4 test compared the typed cause of a real refusal and printed it
with the diagnostic's package, file, byte range and complete text. Nothing here
infers a reason from message text. Causes forwarded unchanged from the target
contract or the shared integer primitive are listed separately and never count
toward an owned reason. Package-command witnesses must agree with the independently checked CLI diagnostic.
"""
import re
import unicodedata
from pathlib import Path

from diagnostic_fields import readable, unescape


SUITE = "L4"
DECLARATION = "layers/04_typed-trees-to-prepared-trees/src/reasons.rs"
# The one test trusted to print the compiled roster: (libtest target, test name).
INVENTORY = ("tests/preparation_reasons.rs", "declared_preparation_reasons")
INVENTORY_TAG = "OMEGA_PREPARATION_REASONS"
REASON_TAG = "OMEGA_PREPARATION_REASON"
FORWARDED_TAG = "OMEGA_PREPARATION_FORWARDED"
PREFIX = "OMEGA_PREPARATION_"
FORWARDED = ("TargetLayout", "IntegerPrimitive")
WITNESS_TARGET = re.compile(r"tests/[^/]+\.rs|unittests src/lib\.rs")
NAME = re.compile(r"[A-Z][A-Za-z0-9]*")
KEY = "preparation_reason_records"


def display(text):
    """Percent-escape `%`, control characters and line separators, like spec_corpus::diagnostic_field."""
    out = []
    for character in text:
        if character == "%" or unicodedata.category(character) == "Cc" or character in "\u2028\u2029":
            out.extend(f"%{byte:02X}" for byte in character.encode("utf-8"))
        else:
            out.append(character)
    return "".join(out)


def field(raw):
    """Decode one escaped record field. Every character Rust escapes must arrive escaped."""
    if bad := next((c for c in raw if unicodedata.category(c) == "Cc" or c in "\u2028\u2029"), None):
        raise ValueError(f"unescaped control character {bad!r}")
    return unescape(raw).decode("utf-8")


def rust_string(literal):
    escapes = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", "\\": "\\", "\"": "\"", "'": "'"}

    def replace(match):
        if match[1] in escapes:
            return escapes[match[1]]
        if code := re.fullmatch(r"u\{([0-9A-Fa-f]{1,6})\}", match[1]):
            return chr(int(code[1], 16))
        raise ValueError(f"unsupported escape \\{match[1]}")

    return re.sub(r"\\(u\{[^}]*\}|.)", replace, literal)


def declared(path=DECLARATION):
    """Reason name -> rendered text, in declaration order. Any unfamiliar line is refused."""
    source = Path(path).read_text(encoding="utf-8")
    blocks = re.findall(r"^preparation_reasons! \{\n(.*?)^\}", source, re.M | re.S)
    if len(blocks) != 1:
        raise ValueError(f"{path}: expected one preparation_reasons! declaration, found {len(blocks)}")
    reasons = {}
    for line in blocks[0].splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        match = re.fullmatch(r'([A-Z][A-Za-z0-9]*) => "((?:[^"\\]|\\.)*)",', stripped)
        if not match or match[1] in reasons:
            raise ValueError(f"{path}: unrecognized or repeated preparation reason: {stripped}")
        reasons[match[1]] = rust_string(match[2])
    if not reasons:
        raise ValueError(f"{path}: no preparation reasons")
    return reasons


def _state(report):
    return report.setdefault(KEY, {"inventory": None, "inventory_errors": 0, "witnesses": [], "forwarded": [], "invalid_tests": []})


def record(report, suite, target, capture, line):
    """Validate one OMEGA_PREPARATION_* line printed while collecting `suite`."""
    state = _state(report)
    target = target.replace("\\", "/")
    test = capture[1] if capture else None
    tag = line.split("\t", 1)[0]
    try:
        if tag == INVENTORY_TAG:
            fields = line.split("\t")
            entries = fields[1].split(",") if len(fields) == 2 else []
            if not entries or not all(NAME.fullmatch(entry) for entry in entries) or len(set(entries)) != len(entries):
                raise ValueError("malformed or repeated inventory entry")
            if suite != SUITE or (target, test) != INVENTORY:
                raise ValueError(f"inventory must come from {SUITE} {INVENTORY[0]}::{INVENTORY[1]}")
            if state["inventory"] is not None or state["inventory_errors"]:
                raise ValueError("repeated inventory")
            state["inventory"] = entries
            return
        if tag not in (REASON_TAG, FORWARDED_TAG):
            raise ValueError("unknown preparation record")
        direct = suite == SUITE and WITNESS_TARGET.fullmatch(target)
        package_case = suite == "CLI" and target == "tests/preparation_cases.rs" and tag == REASON_TAG
        if capture is None or not (direct or package_case):
            raise ValueError(f"witness must be printed inside a captured {SUITE} test")
        fields = line.split("\t")
        if tag == REASON_TAG:
            if len(fields) != 9:
                raise ValueError("expected 9 fields")
            _, expected, actual, *location = fields
            if not NAME.fullmatch(expected) or actual != expected:
                raise ValueError("typed cause differs from the expected reason")
            kind = "reason"
        else:
            if len(fields) != 8:
                raise ValueError("expected 8 fields")
            _, actual, *location = fields
            if actual not in FORWARDED:
                raise ValueError(f"unknown forwarded cause {actual!r}")
            kind = "forwarded"
        package, file, start, end, size, message = location
        package, file, message = field(package), field(file), field(message)
        if not all(value.isdecimal() and value.isascii() for value in (start, end, size)):
            raise ValueError("non-decimal range or length")
        start, end = int(start), int(end)
        if start > end:
            raise ValueError("inverted byte range")
        if not message or len(message.encode("utf-8")) != int(size):
            raise ValueError("empty or incomplete diagnostic text")
        # An unlocated diagnostic names no package or file and has an empty range at zero.
        if not (package and file) and (package or file or end):
            raise ValueError("partial source location")
        row = {"suite": suite, "target": target, "test": test, "cause": actual, "package": package,
               "file": file, "byte_start": start, "byte_end": end, "reason": message}
        rows = state["witnesses" if kind == "reason" else "forwarded"]
        if row in rows:
            raise ValueError("duplicate record in one test")
        rows.append(row)
    except (ValueError, UnicodeDecodeError) as error:
        where = f"{target}::{test}" if test else f"{target} outside a captured test"
        report["measurement_errors"].append(f"{suite}: invalid preparation reason record in {where}: {error}: {display(line)}")
        if tag == INVENTORY_TAG:
            state["inventory_errors"] += 1
        elif test is not None and [suite, target, test] not in state["invalid_tests"]:
            state["invalid_tests"].append([suite, target, test])


def measure(report):
    """Return the coverage of every owned reason, or an unmeasured result naming why the roster is untrusted."""
    errors = report["measurement_errors"]
    state = _state(report)
    statuses = {}
    for suite in (SUITE, "CLI"):
        for test in report["suites"].get(suite, {}).get("tests", ()):
            statuses.setdefault((suite, test["target"].replace("\\", "/"), test["test"]), []).append(test["status"])
    problems = []
    roster = state["inventory"]
    if roster is None or state["inventory_errors"]:
        problems.append(f"no valid PreparationReason::ALL inventory from {INVENTORY[0]}::{INVENTORY[1]}")
    elif statuses.get((SUITE, *INVENTORY)) != ["ok"]:
        problems.append(f"PreparationReason::ALL inventory test {INVENTORY[1]}: {statuses.get((SUITE, *INVENTORY)) or 'absent'}")
    try:
        texts = declared()
    except (OSError, ValueError) as error:
        problems.append(f"cannot read the preparation_reasons! declaration: {error}")
        texts = None
    if roster is not None and texts is not None and roster != list(texts):
        absent, extra = [r for r in texts if r not in roster], [r for r in roster if r not in texts]
        problems.append("PreparationReason::ALL disagrees with the preparation_reasons! declaration"
                        + (f"; not in ALL: {', '.join(absent)}" if absent else "")
                        + (f"; not declared: {', '.join(extra)}" if extra else "")
                        + ("; order differs" if not absent and not extra else ""))
    forwarded = credited(report, state, statuses, state["forwarded"], None, None)
    if problems:
        errors.extend(problems)
        return {"measured": False, "problems": problems, "reasons": {}, "missing": [], "forwarded": forwarded}
    reasons = {name: {"text": texts[name], "state": "missing", "witnesses": []} for name in roster}
    observed_cli = {row["test"] for row in state["witnesses"] if row["suite"] == "CLI"}
    for test, rule in report.get("rule_diagnostics", {}).get("L4", {}).items():
        if rule["verified"] and test not in observed_cli:
            errors.append(f"CLI::{test}: missing typed preparation cause for the verified package refusal")
    for row in credited(report, state, statuses, state["witnesses"], reasons, texts):
        entry = reasons[row["cause"]]
        if row["status"] == "ok":
            entry["state"] = "verified"
            entry["witnesses"].append(row)
        elif entry["state"] == "missing":
            entry["state"] = "witness in a failed test"
    missing = [name for name, entry in reasons.items() if entry["state"] != "verified"]
    return {"measured": True, "problems": [], "reasons": reasons, "missing": missing, "forwarded": forwarded}


def credited(report, state, statuses, rows, reasons, texts):
    """Rows from tests whose records are all valid. A failing test's rows keep status FAILED and never verify."""
    errors = report["measurement_errors"]
    invalid = {tuple(key) for key in state["invalid_tests"]}
    by_test = {}
    for row in rows:
        by_test.setdefault((row["suite"], row["target"], row["test"]), []).append(row)
    accepted = []
    for (suite, target, test), group in by_test.items():
        if (suite, target, test) in invalid:
            continue
        status = statuses.get((suite, target, test), [])
        if status not in (["ok"], ["FAILED"]):
            errors.append(f"{suite}::{target}::{test}: preparation cause records require one test result, found {status or 'none'}")
            continue
        if suite == "CLI":
            rule = report.get("rule_diagnostics", {}).get("L4", {}).get(test, {})
            observed = rule.get("observation") or {}
            if (len(group) != 1 or not rule.get("verified") or group[0]["package"] != "root"
                    or any(group[0][key] != observed.get(key) for key in ("file", "byte_start", "byte_end", "reason"))):
                errors.append(f"CLI::{test}: typed preparation cause differs from the verified package refusal")
                continue
        if reasons is not None:
            unknown = sorted({row["cause"] for row in group} - reasons.keys())
            mismatched = [row["cause"] for row in group if row["cause"] in texts and texts[row["cause"]] not in row["reason"]]
            if unknown or mismatched:
                errors.append(f"{suite}::{target}::{test}: " + "; ".join(
                    ([f"undeclared PreparationReason {', '.join(unknown)}"] if unknown else [])
                    + ([f"diagnostic omits the declared text of {', '.join(mismatched)}"] if mismatched else [])))
                continue
        accepted.extend({**row, "status": status[0]} for row in group)
    return accepted


def blocks(result):
    """Readiness fails on a broken roster and on any owned reason without a verified witness."""
    return result is not None and (not result["measured"] or bool(result["missing"]))


def _site(row):
    origin = "/".join(readable(part) for part in (row["package"], row["file"]) if part) or "<unlocated>"
    return f"{row['suite']} · {row['target']}::{row['test']} · {origin} · bytes {row['byte_start']}..{row['byte_end']} · {readable(row['reason'])}"


def details(result):
    lines = ["## Preparation reasons", "",
             "L4 supplies the list through `PreparationReason::ALL`. Each witness compares the cause, package, file, byte range and full diagnostic. "
             "CLI witnesses also require the package command to return that diagnostic for the same input. "
             "Tests in `typed_input_diagnostics.rs` prepare a healthy typed package, then change one input record to exercise L4 directly.", ""]
    if not result["measured"]:
        lines += ["Not measured. The roster of owned reasons could not be trusted:", "", *[f"- {display(p)}" for p in result["problems"]], ""]
    else:
        verified = len(result["reasons"]) - len(result["missing"])
        lines += [f"{verified} of {len(result['reasons'])} owned reasons have a verified typed witness. Unverified: {len(result['missing'])}.", ""]
        for name, entry in result["reasons"].items():
            lines.append(f"- {name} · {entry['state']} · {display(entry['text'])}")
            lines += [f"  - {_site(row)}" for row in entry["witnesses"]]
        lines.append("")
    lines += ["### Forwarded causes", "",
              "These causes come from the target contract or the shared integer primitive. They never count toward an owned reason.", ""]
    rows = [row for row in result["forwarded"] if row["status"] == "ok"]
    lines += [f"- {row['cause']} · {_site(row)}" for row in rows] or ["No passing test recorded a forwarded cause."]
    lines.append("")
    return lines


def summary(result):
    if result is None:
        return []
    forwarded = [row for row in result["forwarded"] if row["status"] == "ok"]
    tail = (f" Forwarded target-layout or integer-primitive causes, not counted: {len(forwarded)}."
            " [Evidence for each reason](rule-diagnostics.md#preparation-reasons)")
    if not result["measured"]:
        return [f"- L4 · owned preparation reasons not measured: {'; '.join(display(p) for p in result['problems'])}.{tail}"]
    total = len(result["reasons"])
    line = f"- L4 · {total - len(result['missing'])}/{total} owned preparation reasons have a verified typed witness."
    return [line + tail]
