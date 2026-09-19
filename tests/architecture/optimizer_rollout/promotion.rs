//! Owner-review evidence required by every nonexperimental exact rule, plus
//! the schema and identity checks that keep a staged record in step with its
//! still-`Experimental` inventory row.

use std::collections::BTreeMap;
use std::fs;

use crate::Audit;
use crate::inventory::ReleaseRow;

/// The record schema's value-bearing fields. A record must carry every label
/// even while its evidence accrues: an absent field is a malformed record,
/// not a pending one.
const PROMOTION_FIELDS: &[&str] = &[
    "Approved status:",
    "Owner approval:",
    "Semantic and corruption evidence:",
    "Differential evidence:",
    "Determinism and bounded-work evidence:",
    "Target matrix evidence:",
    "Measurement evidence:",
];

pub(super) fn check(audit: &mut Audit, published: &BTreeMap<String, ReleaseRow>) {
    let records = check_record_inventory(audit, published);
    for (name, row) in published {
        let relative = format!("{}/{name}.md", super::PROMOTION_ROOT);
        match (row.status.as_str(), records.get(name)) {
            ("Recommended" | "Default", Some(contents)) => audit.violations.extend(
                record_defects(name, &row.status, contents)
                    .into_iter()
                    .map(|defect| format!("optimizer promotion record {relative} {defect}")),
            ),
            ("Recommended" | "Default", None) => {
                audit.violations.insert(format!(
                    "optimizer rule `{name}` is {} without owner-reviewed promotion record {relative}",
                    row.status
                ));
            }
            ("Experimental", Some(contents)) => audit.violations.extend(
                staged_record_defects(name, contents)
                    .into_iter()
                    .map(|defect| format!("optimizer promotion record {relative} {defect}")),
            ),
            // Unknown statuses are already reported by the inventory check.
            _ => {}
        }
    }
}

fn check_record_inventory(
    audit: &mut Audit,
    published: &BTreeMap<String, ReleaseRow>,
) -> BTreeMap<String, String> {
    let root = audit.repository.join(super::PROMOTION_ROOT);
    let mut records = BTreeMap::new();
    let Ok(entries) = fs::read_dir(&root) else {
        audit.violations.insert(format!(
            "cannot read optimizer promotion-record directory {}",
            super::PROMOTION_ROOT
        ));
        return records;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md")
            || path.file_name().and_then(|name| name.to_str()) == Some("README.md")
        {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !published.contains_key(name) {
            audit.violations.insert(format!(
                "optimizer promotion record `{}` does not name a canonical published exact rule",
                path.display()
            ));
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(contents) => {
                records.insert(name.to_owned(), contents);
            }
            Err(_) => {
                audit.violations.insert(format!(
                    "cannot read optimizer promotion record `{}`",
                    path.display()
                ));
            }
        }
    }
    records
}

fn record_defects(name: &str, status: &str, contents: &str) -> Vec<String> {
    let mut defects = Vec::new();
    for expected in [
        format!("Exact rule: {name}"),
        format!("Approved status: {status}"),
        format!("Rollback: --disable-optimization {name}"),
    ] {
        if !contents
            .lines()
            .map(normalize_record_line)
            .any(|line| line == expected)
        {
            defects.push(format!("lacks exact `{expected}`"));
        }
    }
    for field in PROMOTION_FIELDS
        .iter()
        .filter(|field| **field != "Approved status:")
    {
        if !contents
            .lines()
            .map(normalize_record_line)
            .any(|line| completed_record_field(line, field))
        {
            defects.push(format!("lacks completed `{field}`"));
        }
    }
    defects
}

/// A staged record sits in `promotions/` while its inventory row remains
/// `Experimental`: it accumulates evidence ahead of a promotion leg, so its
/// values may stay `PENDING`. Its identity legs must already be exact, every
/// schema field must be present, and no completed `Approved status:` may
/// appear — the inventory change the approval authorizes has not happened, so
/// a completed approval would be out of step with the row.
fn staged_record_defects(name: &str, contents: &str) -> Vec<String> {
    let mut defects = Vec::new();
    for expected in [
        format!("Exact rule: {name}"),
        format!("Rollback: --disable-optimization {name}"),
    ] {
        if !contents
            .lines()
            .map(normalize_record_line)
            .any(|line| line == expected)
        {
            defects.push(format!("lacks exact `{expected}`"));
        }
    }
    for field in PROMOTION_FIELDS {
        if !contents
            .lines()
            .map(normalize_record_line)
            .any(|line| line.starts_with(field))
        {
            defects.push(format!("lacks `{field}` field"));
        }
    }
    if contents
        .lines()
        .map(normalize_record_line)
        .any(|line| completed_record_field(line, "Approved status:"))
    {
        defects.push(
            "asserts a completed `Approved status:` before the inventory row leaves `Experimental`"
                .to_owned(),
        );
    }
    defects
}

fn normalize_record_line(line: &str) -> &str {
    line.trim().trim_start_matches('-').trim()
}

fn completed_record_field(line: &str, field: &str) -> bool {
    let Some(value) = line.strip_prefix(field).map(str::trim) else {
        return false;
    };
    !(value.is_empty() || value.contains("PENDING") || value.contains('<') && value.contains('>'))
}

#[test]
fn evidence_rejects_empty_pending_and_template_values() {
    assert!(!completed_record_field(
        "Owner approval:",
        "Owner approval:"
    ));
    assert!(!completed_record_field(
        "Owner approval: PENDING",
        "Owner approval:"
    ));
    assert!(!completed_record_field(
        "Owner approval: <owner, review, date>",
        "Owner approval:"
    ));
    assert!(completed_record_field(
        "Owner approval: compiler-owner, review 42, 2026-08-31",
        "Owner approval:"
    ));
}

#[test]
fn promotion_record_requires_exact_identity_and_completed_evidence() {
    let valid = "\
- Exact rule: ControlFlowCleanup
- Approved status: Recommended
- Owner approval: compiler-owner, review 42, 2026-08-31
- Semantic and corruption evidence: test run 1
- Differential evidence: corpus run 1
- Determinism and bounded-work evidence: test run 2
- Target matrix evidence: matrix run 1
- Measurement evidence: benchmark v1
- Rollback: --disable-optimization ControlFlowCleanup
";
    assert!(record_defects("ControlFlowCleanup", "Recommended", valid).is_empty());

    let incomplete = valid
        .replace(
            "Exact rule: ControlFlowCleanup",
            "Exact rule: CopyPropagation",
        )
        .replace(
            "Differential evidence: corpus run 1",
            "Differential evidence: PENDING",
        );
    let defects = record_defects("ControlFlowCleanup", "Recommended", &incomplete);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("Exact rule: ControlFlowCleanup"))
    );
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("Differential evidence:"))
    );
}

#[test]
fn staged_record_keeps_schema_while_pending_and_rejects_early_approval() {
    let staged = "\
- Exact rule: ControlFlowCleanup
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: evidence-matrix legs
- Differential evidence: PENDING
- Determinism and bounded-work evidence: determinism and budget legs
- Target matrix evidence: PENDING
- Measurement evidence: PENDING
- Rollback: --disable-optimization ControlFlowCleanup
";
    assert!(staged_record_defects("ControlFlowCleanup", staged).is_empty());

    let approved_early = staged.replace("Approved status: PENDING", "Approved status: Recommended");
    let defects = staged_record_defects("ControlFlowCleanup", &approved_early);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("Approved status:")),
        "missing early-approval defect; saw {defects:?}"
    );

    let wrong_identity = staged.replace(
        "Exact rule: ControlFlowCleanup",
        "Exact rule: CopyPropagation",
    );
    let defects = staged_record_defects("ControlFlowCleanup", &wrong_identity);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("Exact rule: ControlFlowCleanup")),
        "missing identity defect; saw {defects:?}"
    );

    let missing_field = staged.replace("- Measurement evidence: PENDING\n", "");
    let defects = staged_record_defects("ControlFlowCleanup", &missing_field);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("`Measurement evidence:`")),
        "missing schema-field defect; saw {defects:?}"
    );
}
