"""Read the explicit L3 `TypeError` call-site inventory and write typing-sites.md.

A row is one explicit constructor call in production L3 source. It is not a distinct typing reason and
not runtime coverage. The unsupported-form fallback and the pending marker are not rules.
"""
from collections import Counter
import os
from pathlib import Path
import re
import subprocess

from diagnostic_fields import readable, unescape

SOURCE = "layers/03_symbol-resolved-trees-to-typed-trees/src"
FIXTURES = Path(SOURCE).parent / "fixtures"
GOLDEN_TEST = "typed_fixtures_match_handwritten_expectations"
COMMAND = ["mbx", "run", "--locked", "-p", "spec-corpus", "--features", "typing-sites", "--bin", "typing_diagnostic_sites"]
HEADER = ["#typing-diagnostic-sites\tv1", "path\tline\tcolumn\tscope\tconstructor\tclass\ttext"]
FOOTER = re.compile(r"#end\tsites=(\d+)\tfiles=(\d+)\texcluded_items=(\d+)")
CLASSES = {
    "literal": "Literal messages",
    "template": "Format templates",
    "dynamic": "Dynamic or forwarded messages",
    "unsupported": "Unsupported-form fallbacks (not rules)",
    "pending": "Pending static evaluation (not rules)",
    "unclassified": "Needs review",
    "malformed": "Files the inventory could not parse",
}
LIMITS = ("Names are not resolved: aliases, `Self::` calls and imported constructors are missed. "
          "Only `test` is read from `cfg`. Macro bodies are inspected only when they parse as expressions. "
          "Generated code and the runtime value of dynamic messages are not seen.")


def parse(text):
    """Return the inventory and its errors; invalid rows are reported and skipped."""
    lines, errors, rows = text.splitlines(), [], []
    inventory = {"rows": rows, "files": 0, "excluded_items": 0, "errors": errors}
    if lines[:2] != HEADER:
        errors.append("missing or changed header")
        return inventory
    footer = FOOTER.fullmatch(lines[-1]) if len(lines) > 2 else None
    if not footer:
        errors.append("missing or malformed footer")
    body = lines[2:-1] if footer else lines[2:]
    for number, line in enumerate(body, 3):
        fields = line.split("\t")
        try:
            if len(fields) != 7:
                raise ValueError(f"{len(fields)} fields")
            path, row_line, column, scope, constructor, kind, message = fields
            if not (row_line.isdigit() and column.isdigit() and int(row_line) > 0 and int(column) > 0):
                raise ValueError("line and column must be positive integers")
            if kind not in CLASSES or not re.fullmatch(r"-|[A-Za-z_][A-Za-z0-9_]*", constructor):
                raise ValueError(f"unknown class {kind!r} or constructor {constructor!r}")
            decoded = [unescape(value).decode("utf-8") for value in (path, scope, message)]
        except (ValueError, UnicodeDecodeError) as error:
            errors.append(f"row {number}: {error}: {readable(line)}")
            continue
        rows.append({"path": decoded[0], "line": int(row_line), "column": int(column), "scope": decoded[1],
                     "constructor": constructor, "class": kind, "text": decoded[2]})
    keys = [(row["path"], row["line"], row["column"]) for row in rows]
    if keys != sorted(keys) or len(set(keys)) != len(keys):
        errors.append("rows are unsorted or repeat a location")
    if footer:
        inventory["files"], inventory["excluded_items"] = int(footer[2]), int(footer[3])
        if not rows or not inventory["files"] or len({r["path"] for r in rows}) > inventory["files"]:
            errors.append("the listed sites do not account for a nonempty set of Rust source files")
        if int(footer[1]) != len(body):
            errors.append(f"footer counts {footer[1]} sites but {len(body)} rows were listed")
    errors.extend(f"{row['path']}:{row['line']}: unparsed source: {row['text']}" for row in rows if row["class"] == "malformed")
    return inventory


def source_link(path, directory):
    try:
        return Path(os.path.relpath(Path(SOURCE) / path, directory)).as_posix()
    except ValueError:  # Another drive: fall back to the workspace-relative path.
        return f"{SOURCE}/{path}"


def record_return(report, stage, target, capture, line):
    """Record a final error returned to the shared L3 test helper."""
    target = target.replace("\\", "/")
    try:
        if stage != "L3" or not capture or not re.fullmatch(r"tests/[^/]+\.rs", target):
            raise ValueError("outside a captured L3 integration test")
        tag, fixture, path, rust_line, col, phase, package, file, start, end, size, message = line.split("\t")
        if tag != "OMEGA_TYPING_RETURN" or phase not in {"refused", "pending"}:
            raise ValueError("unknown record or result phase")
        path, package, file, message = [unescape(v).decode("utf-8") for v in (path, package, file, message)]
        if fixture and ((target, capture[1]) != ("tests/expected.rs", GOLDEN_TEST) or not re.fullmatch(r"(?:prepared/)?reject/[a-zA-Z0-9_-]+\.omg", fixture)):
            raise ValueError("fixture observation outside its golden test or rejection directory")
        rust_line, col, start, end = map(int, (rust_line, col, start, end))
        if not path or min(rust_line, col) < 1 or not 0 <= start <= end or len(message.encode()) != int(size) or not message:
            raise ValueError("invalid origin, range or message length")
        if not (package and file) and (package or file or end):
            raise ValueError("incomplete program location")
        report.setdefault("typing_returns", []).append({"target": target, "test": capture[1], "phase": phase, "fixture": fixture,
            "package": package, "file": file, "start": start, "end": end, "reason": message,
            "compiler_site": {"path": path, "line": rust_line, "column": col}})
    except ValueError as error:
        report["measurement_errors"].append(f"{stage} {target}: invalid returned typing diagnostic: {error}")
        if stage == "L3" and capture:
            key = [target, capture[1]]
            invalid = report.setdefault("invalid_typing_returns", [])
            if key not in invalid:
                invalid.append(key)


def check_golden(result, path):
    """Reconstruct the handwritten expectation from the actual returned fields and source bytes."""
    if (result["phase"], result["package"], result["file"]) != ("refused", "game", "fixture.omg"):
        return False
    data = path.read_bytes()
    start, end = result["start"], result["end"]
    if not 0 <= start <= end <= len(data):
        return False
    line = data.count(b"\n", 0, start) + 1
    column = start - data.rfind(b"\n", 0, start)
    selected = data[start:end].decode("utf-8")
    actual = f"refusal | {line}:{column} | {selected} | {result['reason']}\n"
    return actual.encode() == path.with_suffix(".expected").read_bytes()


def attach_witnesses(inventory, report, fixture_root=FIXTURES):
    """A Rust site can be reached by many tests. Only the package twins check the complete CLI diagnostic."""
    sites = {(r["path"], r["line"], r["column"]): r for r in inventory["rows"]}

    def find_site(origin, name, phase):
        _, separator, path = ("/" + origin["path"]).rpartition(f"/{SOURCE}/")
        site = sites.get((path, origin["line"], origin["column"])) if separator else None
        if site is None:
            inventory["errors"].append(f"{name}: returned diagnostic originated outside the inventory at {origin['path']}:{origin['line']}:{origin['column']}")
        elif site["class"] in {"unclassified", "malformed"} or (site["class"] == "pending") != (phase == "pending"):
            inventory["errors"].append(f"{name}: {phase} result points to a {site['class']} site")
        else:
            return site
        return None

    for name, case in report.get("rule_diagnostics", {}).get("L3", {}).items():
        if not case["verified"]:
            continue
        observation = case["observation"]
        site = find_site(observation["compiler_site"], name, "refused")
        if site is None:
            continue
        if site["class"] == "literal" and site["text"] != observation["reason"]:
            inventory["errors"].append(f"{name}: returned reason disagrees with the literal at its Rust origin")
        else:
            site.setdefault("package_tests", []).append(name)

    statuses = {}
    for test in report.get("suites", {}).get("L3", {}).get("tests", []):
        statuses.setdefault((test["target"].replace("\\", "/"), test["test"]), []).append(test["status"])
    invalid = {tuple(key) for key in report.get("invalid_typing_returns", [])}
    goldens = report.get("handoff_observations", {}).get("goldens", {})
    checked_fixtures = {}
    for result in report.get("typing_returns", []):
        target, test = result["target"], result["test"]
        outcomes = statuses.get((target, test), [])
        result["test_status"] = outcomes[0] if len(outcomes) == 1 else ("repeated" if outcomes else "absent")
        if result["test_status"] == "absent":
            inventory["errors"].append(f"{target}::{test}: no named result for the returned typing diagnostic")
        if result["test_status"] != "ok" or (target, test) in invalid:
            continue
        name = f"{target}::{test}"
        site = find_site(result["compiler_site"], name, result["phase"])
        if site is not None and name not in site.setdefault("stage_tests", []):
            site["stage_tests"].append(name)
        fixture = result["fixture"]
        if site is not None and fixture:
            checked_fixtures.setdefault(fixture, []).append((site, result))
    if statuses.get(("tests/expected.rs", GOLDEN_TEST)) == ["ok"]:
        paths = {p.relative_to(fixture_root).as_posix(): p for pattern in ["reject/*.omg", "prepared/reject/*.omg"] for p in fixture_root.glob(pattern)}
        for fixture in sorted(paths.keys() | checked_fixtures.keys()):
            observed = checked_fixtures.get(fixture, [])
            try:
                verified = (fixture in paths and len(observed) == 1 and goldens.get("L3/" + fixture) == GOLDEN_TEST
                            and check_golden(observed[0][1], paths[fixture]))
            except (ValueError, OSError):
                verified = False
            if verified:
                observed[0][0].setdefault("fixtures", []).append(fixture)
            else:
                inventory["errors"].append(f"{fixture}: missing or inconsistent typed refusal for the matched golden fixture")


def details(inventory, directory):
    rows, counts = inventory["rows"], Counter(row["class"] for row in inventory["rows"])
    lines = ["# Explicit L3 typing diagnostic call sites", "",
             f"{len(rows)} explicit `TypeError` constructor calls in {inventory['files']} files under `{SOURCE}`; "
             f"{inventory['excluded_items']} test-only items or constructor definitions were excluded. "
             "A stage witness identifies a passing test that received an error from the L3 test helper. A package witness also checks the complete refusal at the direct API and CLI.", "", LIMITS, ""]
    for error in inventory["errors"]:
        lines.append(f"- Inventory error: {readable(error)}")
    lines += ["", "| Constructor class | Sites | Exact diagnostic checked | Returned to a passing test only | No recorded return |",
              "| --- | ---: | ---: | ---: | ---: |"]
    for kind, title in CLASSES.items():
        selected = [r for r in rows if r["class"] == kind]
        if selected:
            exact = sum(bool(r.get("package_tests") or r.get("fixtures")) for r in selected)
            reached_only = sum(
                1 for r in selected if r.get("stage_tests") and not (r.get("package_tests") or r.get("fixtures")))
            lines.append(f"| {title} | {len(selected)} | {exact} | {reached_only} | {len(selected) - exact - reached_only} |")
    for kind, title in CLASSES.items():
        if not counts[kind]:
            continue
        lines += ["", f"## {title} ({counts[kind]})", ""]
        for row in (row for row in rows if row["class"] == kind):
            link = source_link(row["path"], directory)
            text = f" · {readable(row['text'])}" if row["text"] else ""
            lines.append(f"- [{row['path']}:{row['line']}:{row['column']}]({link}#L{row['line']}) · {row['scope']} · {row['constructor']}{text}")
            for name in row.get("package_tests", []):
                lines.append(f"  - Returned to package test `{name}`. [Original input and diagnostic](rule-diagnostics.md#l3)")
            if row.get("stage_tests"):
                lines.append("  - Returned to " + ", ".join(f"`{name}`" for name in row["stage_tests"]))
            if row.get("fixtures"):
                lines.append("  - Checked against handwritten expectations for " + ", ".join(f"`{name}`" for name in row["fixtures"]))
    return lines


def finish(report, directory, environment):
    tsv, log = directory / "typing-sites.tsv", directory / "typing-sites.log"
    try:
        with tsv.open("wb") as out, log.open("wb") as err:
            code = subprocess.run(COMMAND, env=environment, stdin=subprocess.DEVNULL, stdout=out, stderr=err).returncode
    except OSError as error:
        code = 1
        log.write_text(str(error), encoding="utf-8")
    if code:
        inventory = {"rows": [], "files": 0, "excluded_items": 0, "errors": [f"command exited {code}; see typing-sites.log"]}
    else:
        inventory = parse(tsv.read_text(encoding="utf-8", errors="replace"))
    inventory["command"] = {"command": COMMAND, "exit": code}
    if not inventory["errors"]:
        attach_witnesses(inventory, report)
    report["measurement_errors"].extend("typing site inventory: " + error for error in inventory["errors"])
    report["typing_sites"] = inventory
    (directory / "typing-sites.md").write_text("\n".join(details(inventory, directory)) + "\n", encoding="utf-8")


def summary(report):
    inventory = report.get("typing_sites")
    if not inventory:
        return []
    if inventory["command"]["exit"]:
        return ["", f"- L3 call-site inventory unavailable (exit {inventory['command']['exit']}). [Log](typing-sites.log)"]
    counts = Counter(row["class"] for row in inventory["rows"])
    parts = ", ".join(f"{counts[kind]} {kind}" for kind in CLASSES if counts[kind])
    flag = f" {len(inventory['errors'])} inventory errors." if inventory["errors"] else ""
    return ["", f"- L3 · {len(inventory['rows'])} explicit `TypeError` call sites ({parts or 'none'}).{flag} "
            f"The L3 test helper received errors from {sum(bool(r.get('stage_tests')) for r in inventory['rows'])} sites in passing tests. "
            f"Handwritten fixture diagnostics matched {sum(len(r.get('fixtures', [])) for r in inventory['rows'])} returned errors. "
            f"Package tests checked the full diagnostic from {sum(bool(r.get('package_tests')) for r in inventory['rows'])} sites. "
            "[Sites and their witnesses](typing-sites.md)"]
