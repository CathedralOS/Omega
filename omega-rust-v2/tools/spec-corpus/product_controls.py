"""Require each deliberate change to a valid Psi product to break its semantic check."""
TARGET = "tests/negative_controls.rs"
CONTROLS = {
    "branch-target/swapped": "swapped_branch_targets_fail_the_returned_input_witness",
    "scalar-operand/swapped": "swapped_equal_typed_boundary_scalars_fail_the_operand_witness",
    **{f"restoration/{change}/{witness}": "restoration_turned_into_replacement_fails_the_vacancy_witness"
       for change in ("replace-unrestricted", "dispose-live") for witness in ("vacancy", "moved-place")},
    **{f"residual/Pair::{field}/{change}": "residual_cleanup_on_the_transferred_field_fails_the_sibling_witness"
       for field in ("left", "right") for change in ("transferred-field", "whole-root", "omitted")},
    **{f"join-arrival/{change}": "a_normalized_or_redirected_arm_fails_the_join_arrival_witness"
       for change in ("disposed-remainder", "same-field")},
    **{f"premised-selector/{relation}/{change}": "a_premised_element_store_fails_its_witness_without_the_clause_or_its_selected_element"
       for relation, changes in (("equal", ("clause-operator", "store-through-vacated-selector")),
                                 ("distinct", ("clause-operator", "displaced-element-kept"))) for change in changes},
}


def record(report, stage, target, current, line):
    fields = line.split("\t", 3)
    if (len(fields) != 4 or fields[2] != "rejected" or not fields[3].strip() or stage != "L5"
            or target.replace("\\", "/") != TARGET or not current or CONTROLS.get(fields[1]) != current[1]):
        report["measurement_errors"].append(f"unexpected product corruption observation: {line}")
        return
    observed = report.setdefault("product_controls", {})
    if fields[1] in observed:
        report["measurement_errors"].append(f"duplicate product corruption observation: {fields[1]}")
        return
    observed[fields[1]] = {"test": current[1], "rejection": fields[3]}


def finish(report):
    statuses = {test["test"]: test["status"] for test in report["suites"]["L5"]["tests"]
                if test["target"].replace("\\", "/") == TARGET}
    observed = report.setdefault("product_controls", {})
    for label, test in CONTROLS.items():
        verified = label in observed and statuses.get(test) == "ok"
        observed.setdefault(label, {"test": test})["verified"] = verified
        if not verified and statuses.get(test) != "FAILED":
            report["measurement_errors"].append(f"missing product corruption check: {label} · {TARGET}::{test}")
