"""Compare each reader's logged line ranges with the ranges its slice assigned.

Run from anywhere: `python3 verify.py`. It reads `slices/NNN.txt` and
`reports/NNN.md` beside this file and prints how many assigned lines the
reports log as read.

A report logs a read as `READ <file name> <first>-<last>`. Thirteen reports
shortened the file name. ALIASES maps each shortened name to the slice file it
stands for, so a reviewer can see every mapping this check relies on. Without
the table the count is 795,020 of 875,311 lines and 124 of 137 slices.

This checks the logs, not the reading: a logged range shows that the reader
claimed the lines, and nothing here can show that it read them with care.
"""

import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
READ = re.compile(r"READ\s+`?([^\s`]+)`?\s+(\d+)\s*[-–]\s*(\d+)")

# slice -> {name the report used: file name in the slice}
ALIASES = {
    "038": {
        "cleanup_dispositions.rs": "checks_multiplicity_cleanup_dispositions.rs",
        "linear_validation.rs": "checks_multiplicity_linear_validation.rs",
        "permission_events.rs": "checks_multiplicity_permission_events.rs",
    },
    "051": {"seq.rs": "execution_terminal_unit_control_statement_sequence.rs"},
    "052": {"seq.rs": "execution_terminal_unit_control_statement_sequence.rs"},
    "080": {"a_b.rs": "admissibility_and_borrow.rs"},
    "109": {"CE": "contract_entailment.rs"},
    "110": {"DC": "declarations_core.rs"},
    "111": {"dp.rs": "domains_and_predicates.rs"},
    "114": {"F": "expression_types_and_fields.rs"},
    "116": {"F": "frozen_program_and_queries.rs", "L": "lib.rs"},
    "117": {"F": "literals_and_struct_literals.rs"},
    "129": {"rrc": "reference_result_custody.rs"},
    "132": {"F": "type_references_and_wire.rs"},
    "137": {"diff": "other_changes.diff"},
}


def assigned(slice_id):
    """The (file name, first, last) ranges a slice assigns."""
    ranges = []
    with open(os.path.join(HERE, "slices", slice_id + ".txt")) as lines:
        for line in lines:
            if line.startswith("#") or not line.strip():
                continue
            path, first, last = line.rstrip("\n").split("\t")
            ranges.append((os.path.basename(path), int(first), int(last)))
    return ranges


def logged(slice_id, text):
    """The ranges a report logs, grouped by slice file name."""
    aliases = ALIASES.get(slice_id, {})
    reads = {}
    for name, first, last in READ.findall(text):
        name = aliases.get(name, os.path.basename(name))
        reads.setdefault(name, []).append((int(first), int(last)))
    return reads


def uncovered(first, last, reads):
    """How many lines of first..=last no logged range covers."""
    missing = 0
    cursor = first
    for start, end in sorted(reads):
        if end < cursor or start > last:
            continue
        if start > cursor:
            missing += start - cursor
        cursor = max(cursor, min(end, last) + 1)
    if cursor <= last:
        missing += last - cursor + 1
    return missing


def main():
    total = covered = 0
    complete, partial, absent = [], [], []
    for number in range(1, 138):
        slice_id = f"{number:03d}"
        ranges = assigned(slice_id)
        need = sum(last - first + 1 for _, first, last in ranges)
        total += need
        report = os.path.join(HERE, "reports", slice_id + ".md")
        if not os.path.isfile(report):
            absent.append(slice_id)
            continue
        with open(report, encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        reads = logged(slice_id, text)
        got = sum(
            (last - first + 1) - uncovered(first, last, reads.get(name, []))
            for name, first, last in ranges
        )
        covered += got
        finished = "## Surprises" in text and "NOT READ" in text
        if got == need and finished:
            complete.append(slice_id)
        else:
            partial.append(f"{slice_id}({got}/{need})")
    print(f"lines assigned {total}; lines logged as read {covered}")
    print(
        f"slices fully logged and finished: {len(complete)}; "
        f"partial: {len(partial)}; no report: {len(absent)}"
    )
    if partial:
        print("partial:", " ".join(partial))
    if absent:
        print("no report:", " ".join(absent))


if __name__ == "__main__":
    main()
