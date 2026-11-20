"""Package CLI refusals exercised for rules in the resolver, typer and preparation contracts."""
import csv
import os
from pathlib import Path
import re
import preparation_reason_evidence
import resolver_kind_evidence
from diagnostic_fields import unescape
from handoff_evidence import DOCUMENTS


BINARIES = {"L2": "resolver_cases", "L3": "typing_cases", "L4": "preparation_cases"}
PRODUCERS = {"L2": {"resolver"}, "L3": {"typer"}, "L4": {"preparation", "preparation (target input)"}}


def registrations():
    path = Path(__file__).with_name("diagnostic_rules.tsv")
    with path.open(encoding="utf-8", newline="") as file:
        reader = csv.reader(file, delimiter="\t", quoting=csv.QUOTE_NONE)
        if next(reader, None) != ["layer", "test", "rule"]:
            raise ValueError(f"{path.name}: invalid header")
        rows = {}
        for stage, test, rule in reader:
            if stage not in BINARIES or not test or not rule or (stage, test) in rows:
                raise ValueError(f"{path.name}: invalid diagnostic registration {stage} {test} {rule}")
            rows[stage, test] = rule
    if set(stage for stage, _ in rows) != BINARIES.keys():
        raise ValueError(f"{path.name}: missing layer")
    return rows


def measure(report, log, registered):
    errors = report["measurement_errors"]
    tests = {}
    for row in report["suites"]["CLI"]["tests"]:
        key = row["target"].replace("\\", "/"), row["test"]
        tests.setdefault(key, []).append(row["status"])
    target = current = ""
    observed = {}
    invalid = set()
    for line in log.splitlines():
        if header := re.match(r"^\s*Running (.+?) \(", line):
            target, current = header[1].replace("\\", "/"), ""
        elif captured := re.fullmatch(r"---- (.+?) stdout ----", line):
            current = captured[1]
        elif line in {"successes:", "failures:"} or line.startswith(("test result:", "   Doc-tests ")):
            current = ""
        elif line.startswith("OMEGA_RULE_DIAGNOSTIC"):
            try:
                tag, stage, test, rule, producer, file, start, end, row, col, upstream, *origin, length, encoded = line.split("\t")
                compiler_site = None
                if stage == "L3":
                    path, rust_line, rust_column = origin
                    if not path or int(rust_line) < 1 or int(rust_column) < 1:
                        raise ValueError("invalid Rust diagnostic origin")
                    compiler_site = {"path": unescape(path).decode("utf-8"), "line": int(rust_line), "column": int(rust_column)}
                elif origin:
                    raise ValueError("unexpected Rust diagnostic origin")
                reason = unescape(encoded).decode("utf-8")
                if len(reason.encode("utf-8")) != int(length):
                    raise ValueError("incomplete diagnostic text")
                key = stage, test
                if (tag != "OMEGA_RULE_DIAGNOSTIC" or registered.get(key) != rule or producer not in PRODUCERS[stage]
                        or target != f"tests/{BINARIES[stage]}.rs" or current != test or key in observed):
                    raise ValueError("wrong owner, contract entry, or emitting test; or repeated observation")
                if not file or not reason or not (0 <= int(start) <= int(end) and int(row) >= 1 and int(col) >= 1):
                    raise ValueError("invalid source location or empty reason")
                if upstream != {"L2": "parsed", "L3": "resolved", "L4": "typed"}[stage] and not (
                        stage == "L4" and upstream.startswith("pending ") and upstream[8:].strip()):
                    raise ValueError("unexpected preceding stage or empty evaluation request")
                observed[key] = {"file": file, "byte_start": int(start), "byte_end": int(end), "line": int(row),
                                 "byte_column": int(col), "input": upstream, "producer": producer, "reason": reason}
                if compiler_site is not None:
                    observed[key]["compiler_site"] = compiler_site
            except (KeyError, ValueError) as error:
                errors.append(f"invalid contract diagnostic in {target}::{current}: {error}: {line}")
                # The harness identifies the emitter even when the record names a different test.
                invalid.add((target, current))
    results = {}
    for (stage, test), prefix in registered.items():
        target = f"tests/{BINARIES[stage]}.rs"
        rule = [row for row in report["handoff_rules"][stage] if row["rule"].startswith(prefix)]
        if len(rule) != 1:
            errors.append(f"{stage}::{test}: expected one contract rule starting {prefix!r}, found {len(rule)}")
            continue
        statuses = tests.get((target, test), [])
        status = statuses[0] if len(statuses) == 1 else ("repeated" if statuses else "absent")
        site = observed.get((stage, test))
        verified = status == "ok" and site is not None and (target, test) not in invalid
        if not verified and status != "FAILED":
            errors.append(f"{stage}::{test}: missing verified contract diagnostic (test {status})")
        results.setdefault(stage, {})[test] = {"rule": rule[0]["rule"], "contract_line": rule[0]["line"], "test": status,
                                              "target": target, "verified": verified, "observation": site}
    return results


def finish(report, directory):
    log = (directory / "frontend-cli.log").read_text(encoding="utf-8", errors="replace")
    try:
        results = measure(report, log, registrations())
    except (ValueError, OSError) as error:
        report["measurement_errors"].append(str(error))
        results = {}
    report["rule_diagnostics"] = results
    lines = ["# Diagnostics for front-end contract rules", "",
             "Each verified case identifies the input that reached the refusing layer. For a computed type, both twins must produce the expected pending L3 request. Preparation and lowering must preserve the owning layer’s refusal, and the repaired package must prepare. Some contract entries describe successful output.", ""]
    for stage, binary in BINARIES.items():
        entries = results.get(stage, {})
        contract_path = Path(os.path.relpath(Path("layers") / DOCUMENTS[stage][0], directory)).as_posix()
        test_path = Path(os.path.relpath(Path("application/tests") / f"{binary}.rs", directory)).as_posix()
        lines.extend([f"## {stage}", "", f"[Layer contract]({contract_path}) · [Original and repaired packages]({test_path})", ""])
        for test, record in entries.items():
            obs = record["observation"]
            lines.append(f"- Contract line {record['contract_line']} · {test} · " + ("verified" if record["verified"] else f"unverified ({record['test']})"))
            if not record["verified"]:
                lines.append(f"  - Rerun with `mbx test --locked -p omega --test {binary} {test} -- --exact --show-output`.")
            if obs:
                reason = obs["reason"].replace("\n", "\n    ")
                lines.append(f"  - Input {obs['input']} · {obs['file']}:{obs['line']}:{obs['byte_column']} · bytes {obs['byte_start']}..{obs['byte_end']} · {obs['producer']} · {reason}")
        covered = {entry["rule"] for entry in entries.values() if entry["verified"]}
        lines += ["", "Rules without a registered, passing CLI refusal in this suite", ""]
        lines += [f"- Line {row['line']} · {row['rule']}" for row in report["handoff_rules"].get(stage, ()) if row["rule"] not in covered]
        lines.append("")
    inventory = resolver_kind_evidence.measure(report)
    report["resolver_diagnostic_kinds"] = inventory
    lines.extend(resolver_kind_evidence.details(inventory, report.get("declared_resolver_kinds", {})))
    reasons = preparation_reason_evidence.measure(report)
    report["preparation_reasons"] = reasons
    lines += ["", *preparation_reason_evidence.details(reasons)]
    (directory / "rule-diagnostics.md").write_text("\n".join(lines), encoding="utf-8")


def summary(report):
    lines = []
    inventory = report.get("resolver_diagnostic_kinds", {})
    if inventory:
        verified = sum(bool(rows) for rows in inventory.values())
        lines.append(f"- L2 · {verified}/{len(inventory)} declared resolver diagnostic kinds have an exact typed witness. "
                     "[Evidence for each kind](rule-diagnostics.md#resolver-diagnostic-kinds)")
    for stage, entries in report.get("rule_diagnostics", {}).items():
        verified = [row for row in entries.values() if row["verified"]]
        lines.append(f"- {stage} · {len(verified)}/{len(entries)} registered package refusals verified across {len({row['rule'] for row in verified})} contract entries.")
    lines.extend(preparation_reason_evidence.summary(report.get("preparation_reasons")))
    if not lines:
        return []
    return ["", "## Diagnostics from resolution through preparation", "", *lines, "",
            "L3 has no complete inventory of typing reasons. L4 measures each declared failure phase and each owned preparation reason separately. "
            "Forwarded target-layout and integer-primitive causes are listed but not counted.",
            "[Source locations and uncovered rules](rule-diagnostics.md) · [Preparation reasons](rule-diagnostics.md#preparation-reasons) · "
            "[Preparation phases](frontend-diagnostics.md#preparation-failure-phases)"]
