"""Package tests and the contract rule and optional typed reason each test checks."""
import csv
from functools import cache
from pathlib import Path
import re


BINARIES = {"L2": "resolver_cases", "L3": "typing_cases", "L4": "preparation_cases"}
TYPING_REASON = re.compile(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*:[A-Z][A-Za-z0-9]*")


@cache
def _entries():
    path = Path(__file__).with_name("diagnostic_rules.tsv")
    rules, reasons = {}, {}
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.reader(source, delimiter="\t", quoting=csv.QUOTE_NONE)
        if next(reader, None) != ["layer", "test", "rule", "typing_reason"]:
            raise ValueError(f"{path.name}: invalid header")
        for fields in reader:
            if len(fields) not in (3, 4):
                raise ValueError(f"{path.name}:{reader.line_num}: expected a rule and optional typing reason")
            stage, test, rule = fields[:3]
            if stage not in BINARIES or not test or not rule or (stage, test) in rules:
                raise ValueError(f"{path.name}:{reader.line_num}: invalid diagnostic registration")
            rules[stage, test] = rule
            if len(fields) == 4:
                if stage != "L3" or not TYPING_REASON.fullmatch(fields[3]):
                    raise ValueError(f"{path.name}:{reader.line_num}: invalid L3 typing reason")
                reasons[test] = fields[3]
    if {stage for stage, _ in rules} != BINARIES.keys():
        raise ValueError(f"{path.name}: missing layer")
    return rules, reasons


def registrations():
    return _entries()[0].copy()


def typing_reasons():
    return _entries()[1]
