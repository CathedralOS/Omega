"""Compare the resolver's declared diagnostic kinds with typed errors from real inputs."""
import re
from diagnostic_fields import unescape


INVENTORY_TEST = "declared_resolver_diagnostic_kinds"
TARGETS = {"CLI": {"tests/resolver_cases.rs"}, "L2": {"tests/compiled_interface_diagnostics.rs", "tests/stage.rs", "tests/stage_input_diagnostics.rs"},
           "L3": {"tests/standard.rs"}}


def record(report, suite, target, capture, line):
    target = target.replace("\\", "/")
    if line.startswith("OMEGA_RESOLVER_INVENTORY"):
        fields = line.split("\t")
        entries = fields[1].split(",") if len(fields) == 2 else []
        roster = {}
        for entry in entries:
            pair = re.fullmatch(r"([A-Z][A-Za-z0-9]*)=([A-Z][A-Za-z0-9]*)", entry)
            if not pair or pair[1] in roster:
                break
            roster[pair[1]] = pair[2]
        if (fields[0] != "OMEGA_RESOLVER_INVENTORY" or suite != "L2" or target != "tests/stage.rs"
                or not capture or capture[1] != INVENTORY_TEST or not roster or len(roster) != len(entries)
                or "declared_resolver_kinds" in report):
            report["measurement_errors"].append(f"{suite}: malformed, duplicate or misplaced resolver kind inventory: {line}")
            report["invalid_resolver_inventory"] = True
        else:
            report["declared_resolver_kinds"] = roster
        return
    try:
        tag, expected, actual, package, file, start, end, size, message = line.split("\t")
        if tag != "OMEGA_RESOLVER_KIND" or not capture or target not in TARGETS.get(suite, ()):
            raise ValueError("wrong emitting test or suite")
        if suite == "L3" and (capture[1] != "raw_stages_cannot_define_sealed_toolchain_operations" or actual != "ReservedPackageIdentity"):
            raise ValueError("wrong toolchain identity witness")
        if not expected or actual != expected:
            raise ValueError("inconsistent kind")
        package, file, reason = (unescape(value).decode("utf-8") for value in (package, file, message))
        if not reason or len(reason.encode("utf-8")) != int(size):
            raise ValueError("incomplete resolver reason")
        start, end = int(start), int(end)
        if not 0 <= start <= end:
            raise ValueError("invalid source range")
    except ValueError as error:
        report["measurement_errors"].append(f"{suite}: invalid resolver kind observation: {error}: {line}")
        if capture:
            report.setdefault("invalid_resolver_witnesses", []).append([suite, target, capture[1]])
        return
    report.setdefault("resolver_kind_records", []).append({
        "suite": suite, "target": target, "test": capture[1], "kind": actual, "package": package,
        "file": file, "byte_start": start, "byte_end": end, "reason": reason,
    })


def measure(report):
    errors = report["measurement_errors"]
    declared = report.get("declared_resolver_kinds", {})
    status = [test["status"] for test in report["suites"]["L2"]["tests"]
              if test["target"].replace("\\", "/") == "tests/stage.rs" and test["test"] == INVENTORY_TEST]
    if not declared or report.get("invalid_resolver_inventory") or status != ["ok"]:
        errors.append("missing verified inventory from NameErrorKind::ALL")
        return {}
    inventory = {kind: [] for kind in sorted(declared)}
    cli = report["rule_diagnostics"].get("L2", {})
    results = {}
    for suite, targets in TARGETS.items():
        for test in report["suites"].get(suite, {}).get("tests", ()):
            target = test["target"].replace("\\", "/")
            if target in targets:
                results.setdefault((suite, target, test["test"]), []).append(test["status"])
    by_test = {}
    invalid = {tuple(key) for key in report.get("invalid_resolver_witnesses", ())}
    for row in report.get("resolver_kind_records", ()):
        by_test.setdefault((row["suite"], row["target"], row["test"]), []).append(row)
    for key, rows in by_test.items():
        suite, _, test = key
        if key in invalid:
            continue
        if results.get(key) != ["ok"]:
            errors.append(f"{suite}::{test}: resolver kind observations require one passing test")
            continue
        if suite == "CLI":
            rule = cli.get(test, {})
            observation = rule.get("observation") or {}
            if (len(rows) != 1 or not rule.get("verified") or rows[0]["package"] != "root"
                    or any(rows[0][field] != observation.get(field) for field in ("file", "byte_start", "byte_end", "reason"))):
                errors.append(f"L2::{test}: typed resolver diagnostic does not match a single verified CLI refusal")
                continue
        unknown = {row["kind"] for row in rows} - inventory.keys()
        if unknown:
            errors.append(f"{suite}::{test}: undeclared NameErrorKind {', '.join(sorted(unknown))}")
            continue
        for row in rows:
            if row not in inventory[row["kind"]]:
                inventory[row["kind"]].append(row)
    for test, rule in cli.items():
        if rule["verified"] and ("CLI", "tests/resolver_cases.rs", test) not in by_test:
            errors.append(f"L2::{test}: missing typed resolver diagnostic for the verified CLI refusal")
    return inventory


def details(inventory, declared):
    lines = ["## Resolver diagnostic kinds", "",
             "The resolver supplies the complete list. Each witness compares the kind, reason and input location. "
             "CLI witnesses also verify the package command. Interface failures name the supplied declaration or module.", ""]
    for kind, rows in inventory.items():
        lines.append(f"- {declared[kind]} · {kind}" + ("" if rows else " · unverified"))
        for row in rows:
            origin = "/".join(part for part in (row["package"], row["file"]) if part) or "<stage input>"
            reason = row["reason"].replace("\n", "\n    ")
            lines.append(f"  - {row['suite']} · {row['target']}::{row['test']} · {origin} · bytes {row['byte_start']}..{row['byte_end']} · {reason}")
    return lines
