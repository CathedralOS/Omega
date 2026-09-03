//! Owner-review evidence required by every nonexperimental exact rule.

use std::collections::BTreeMap;
use std::fs;

use crate::Audit;
use crate::inventory::ReleaseRow;

pub(super) fn check(audit: &mut Audit, published: &BTreeMap<String, ReleaseRow>) {
    check_record_inventory(audit, published);
    for (name, row) in published {
        if matches!(row.status.as_str(), "Recommended" | "Default") {
            check_record(audit, name, &row.status);
        }
    }
}

fn check_record_inventory(audit: &mut Audit, published: &BTreeMap<String, ReleaseRow>) {
    let root = audit.repository.join(super::PROMOTION_ROOT);
    let Ok(entries) = fs::read_dir(&root) else {
        audit.violations.insert(format!(
            "cannot read optimizer promotion-record directory {}",
            super::PROMOTION_ROOT
        ));
        return;
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
        }
    }
}

fn check_record(audit: &mut Audit, name: &str, status: &str) {
    let relative = format!("{}/{name}.md", super::PROMOTION_ROOT);
    let Ok(contents) = fs::read_to_string(audit.repository.join(&relative)) else {
        audit.violations.insert(format!(
            "optimizer rule `{name}` is {status} without owner-reviewed promotion record {relative}"
        ));
        return;
    };
    audit.violations.extend(
        record_defects(name, status, &contents)
            .into_iter()
            .map(|defect| format!("optimizer promotion record {relative} {defect}")),
    );
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
    for field in [
        "Owner approval:",
        "Semantic and corruption evidence:",
        "Differential evidence:",
        "Determinism and bounded-work evidence:",
        "Target matrix evidence:",
        "Measurement evidence:",
    ] {
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
