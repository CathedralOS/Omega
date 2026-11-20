"""Finite L3 reason families. A returned message, Rust site or stage test is not a package witness."""

from diagnostic_fields import readable, unescape
from diagnostic_registrations import TYPING_REASON, typing_reasons

PREFIX = "OMEGA_TYPING_REASON_INVENTORY"
EMITTER = ("tests/typing_reasons.rs", "declared_typing_reasons")


def take_cause(fields):
    if fields and TYPING_REASON.fullmatch(fields[-1]):
        return fields.pop()
    return None


def record(report, stage, target, captured, line):
    try:
        tag, count, *fields = line.split("\t")
        if (tag != PREFIX or stage != "L3" or not captured or (target.replace("\\", "/"), captured[1]) != EMITTER
                or "typing_reason_roster" in report or not fields or 2 * int(count) != len(fields)):
            raise ValueError("wrong emitter, repeated or incomplete roster")
        roster = {}
        for identity, encoded in zip(fields[::2], fields[1::2]):
            if not TYPING_REASON.fullmatch(identity) or identity in roster or not encoded:
                raise ValueError("invalid or repeated reason")
            roster[identity] = unescape(encoded).decode("utf-8")
        report["typing_reason_roster"] = roster
    except ValueError as error:
        report["invalid_typing_reason_roster"] = True
        report["measurement_errors"].append(f"invalid typing reason inventory: {error}")


def attach(inventory, site, observation, test, role):
    """The caller has verified the test result, origin and (for packages) CLI record."""
    cause = observation.get("typing_reason")
    if site["constructor"] == "for_direct_call_pause":
        # Old scanner/log pair: retain its returned-site/text evidence but not current family credit.
        return
    if not cause and site["constructor"] != "for_reason":
        return
    if not cause or site["constructor"] != "for_reason" or observation.get("phase", "refused") != "refused":
        inventory["errors"].append(f"{test}: missing, misplaced or unexpected typing reason: {cause}")
        return
    site.setdefault("reason_returns", []).append({"cause": cause, "reason": observation["reason"], "test": test, "role": role})


def measure(report, inventory):
    roster = report.get("typing_reason_roster", {})
    outcomes = [r["status"] for r in report["suites"]["L3"]["tests"] if (r["target"].replace("\\", "/"), r["test"]) == EMITTER]
    try:
        expected = typing_reasons()
    except (ValueError, OSError) as error:
        inventory["errors"].append(f"cannot verify typing reason registrations: {error}")
        return {"measured": False}
    absent = sorted(set(expected.values()) - roster.keys())
    if not roster or absent or report.get("invalid_typing_reason_roster") or outcomes != ["ok"]:
        inventory["errors"].append("missing valid passing TypingReason::ALL inventory" + (": " + ", ".join(absent) if absent else ""))
        return {"measured": False}
    causes = {key: {"reason": text, "package": [], "stage": []} for key, text in roster.items()}
    for site in inventory["rows"]:
        for row in site.get("reason_returns", []):
            entry = causes.get(row["cause"])
            if row["role"] == "package" and expected.get(row["test"]) != row["cause"]:
                inventory["errors"].append(f"{row['test']}: unexpected typing reason: {row['cause']}")
            elif not entry or entry["reason"] != row["reason"]:
                inventory["errors"].append(f"{row['test']}: undeclared typing cause or inconsistent rendering: {row['cause']}")
            elif row["test"] not in entry[row["role"]]:
                entry[row["role"]].append(row["test"])
    families = {}
    for key, entry in causes.items():
        family, name = key.split(":")
        families.setdefault(family, {})[name] = entry
    return {"measured": True, "families": families}


def blocks(result):
    if result is None:
        return False
    return not result["measured"] or any(not row["package"] for family in result["families"].values() for row in family.values())


def summary(result):
    if not result:
        return []
    if not result["measured"]:
        return ["- L3 finite typing reason inventory unavailable."]
    return [f"- L3 · `{family}`: {sum(bool(row['package']) for row in causes.values())}/{len(causes)} declared reasons have "
            "exact typed package witnesses. [Sibling ledger](typing-sites.md#finite-typing-reasons)"
            for family, causes in result["families"].items()]


def details(result):
    if not result:
        return []
    lines = ["## Finite typing reasons", "", *summary(result), "",
             "Only named package twins credit a sibling. Other passing L3 returns are listed separately. "
             "These families are not an inventory of all typing reasons.", ""]
    for family, causes in result.get("families", {}).items():
        lines += [f"### {family}", ""]
        for name, row in causes.items():
            checked = ", ".join(f"`{test}`" for test in row["package"]) or "missing"
            lines.append(f"- `{name}` · {readable(row['reason'])} · exact package: {checked}")
            if row["stage"]:
                lines.append("  - Stage returns: " + ", ".join(f"`{test}`" for test in row["stage"]))
        lines.append("")
    return lines
