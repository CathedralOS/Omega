//! Owner-review evidence required by every nonexperimental exact rule, plus
//! the schema and identity checks that keep a staged record in step with its
//! still-`Experimental` inventory row.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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

/// The schema fields that carry evidence. A completed value must cite at
/// least one repository artifact, so a promotion leg cannot pass on
/// unverifiable prose. `Approved status` and `Owner approval` record a
/// decision, and the identity and rollback lines are checked exactly, so
/// none of them is an evidence field.
const EVIDENCE_FIELDS: &[&str] = &[
    "Semantic and corruption evidence:",
    "Differential evidence:",
    "Determinism and bounded-work evidence:",
    "Target matrix evidence:",
    "Measurement evidence:",
];

/// Extensions that make a bare filename a repository citation. A backticked
/// span whose leading `::`-free segment contains `/` or ends in one of these
/// is read as `path` or `path::subject` and must resolve in the checkout.
/// Other spans — `Optimization::ALL`, `HOSTED_NATIVE_TARGETS`, flags,
/// version names like `1.5x` — are prose and resolve nothing.
const CITATION_EXTENSIONS: &[&str] = &[
    ".rs", ".omg", ".md", ".toml", ".json", ".yaml", ".yml", ".txt",
];

pub(super) fn check(audit: &mut Audit, published: &BTreeMap<String, ReleaseRow>) {
    let records = check_record_inventory(audit, published);
    for (name, row) in published {
        let relative = format!("{}/{name}.md", super::PROMOTION_ROOT);
        match (row.status.as_str(), records.get(name)) {
            ("Recommended" | "Default", Some(contents)) => audit.violations.extend(
                record_defects(&audit.repository, name, &row.status, contents)
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
                staged_record_defects(&audit.repository, name, contents)
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

fn record_defects(repository: &Path, name: &str, status: &str, contents: &str) -> Vec<String> {
    let mut defects = Vec::new();
    for expected in [
        format!("# {name} Promotion"),
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
    defects.extend(citation_defects(repository, contents));
    defects
}

/// A staged record sits in `promotions/` while its inventory row remains
/// `Experimental`: it accumulates evidence ahead of a promotion leg, so its
/// values may stay `PENDING`. Its identity legs must already be exact, every
/// schema field must be present, and no completed `Approved status:` may
/// appear — the inventory change the approval authorizes has not happened, so
/// a completed approval would be out of step with the row.
fn staged_record_defects(repository: &Path, name: &str, contents: &str) -> Vec<String> {
    let mut defects = Vec::new();
    for expected in [
        format!("# {name} Promotion"),
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
    defects.extend(citation_defects(repository, contents));
    defects
}

fn backticked_spans(line: &str) -> impl Iterator<Item = &str> {
    line.split('`').skip(1).step_by(2)
}

fn repository_citation(span: &str) -> Option<&str> {
    let path = span.split("::").next()?;
    (path.contains('/')
        || CITATION_EXTENSIONS
            .iter()
            .any(|extension| path.ends_with(extension)))
    .then_some(span)
}

/// Resolve one `path` or `path::subject` citation against the checkout: the
/// path must exist, and each `::subject` must appear in the file's text, so
/// a record cannot name a renamed, moved, or imagined artifact.
fn resolve_citation(repository: &Path, citation: &str) -> Result<(), String> {
    let mut parts = citation.split("::");
    let path = parts.next().unwrap_or_default();
    let joined = repository.join(path);
    let metadata = fs::metadata(&joined)
        .map_err(|_| format!("cites `{citation}` but `{path}` is not a repository path"))?;
    if metadata.is_dir() {
        return if parts.next().is_none() {
            Ok(())
        } else {
            Err(format!(
                "cites `{citation}` but directory `{path}` cannot name a subject"
            ))
        };
    }
    let text = fs::read_to_string(&joined)
        .map_err(|_| format!("cites `{citation}` but `{path}` is not a readable text artifact"))?;
    for subject in parts {
        if !text.contains(subject) {
            return Err(format!(
                "cites `{citation}` but `{subject}` does not appear in `{path}`"
            ));
        }
    }
    Ok(())
}

/// Every artifact citation in the record resolves, and every completed
/// evidence field cites at least one. A `PENDING` field names open work and
/// stays exempt; a field completed without a citation is unverifiable prose.
fn citation_defects(repository: &Path, contents: &str) -> Vec<String> {
    let mut defects = Vec::new();
    for line in contents.lines().map(normalize_record_line) {
        for span in backticked_spans(line) {
            if let Some(citation) = repository_citation(span)
                && let Err(defect) = resolve_citation(repository, citation)
            {
                defects.push(defect);
            }
        }
    }
    for field in EVIDENCE_FIELDS {
        let Some(line) = contents
            .lines()
            .map(normalize_record_line)
            .find(|line| line.starts_with(field))
        else {
            continue;
        };
        if !completed_record_field(line, field) {
            continue;
        }
        if !backticked_spans(&line[field.len()..]).any(|span| repository_citation(span).is_some()) {
            defects.push(format!("completed `{field}` cites no repository artifact"));
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

fn fixture_repository(test: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omega_promotion_gate_{}_{}_{}",
        test,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(root.join("evidence")).expect("fixture root");
    std::fs::write(root.join("evidence/rule.rs"), "fn real_test() {}\n").expect("fixture evidence");
    std::fs::create_dir_all(root.join("corpus/case")).expect("fixture corpus");
    root
}

#[test]
fn promotion_record_requires_exact_identity_and_completed_evidence() {
    let repository = fixture_repository("promoted");
    let valid = "\
# ControlFlowCleanup Promotion
- Exact rule: ControlFlowCleanup
- Approved status: Recommended
- Owner approval: compiler-owner, review 42, 2026-08-31
- Semantic and corruption evidence: `evidence/rule.rs::real_test` matrix legs
- Differential evidence: `corpus/case` corpus run 1
- Determinism and bounded-work evidence: `evidence/rule.rs::real_test` determinism leg
- Target matrix evidence: `evidence/rule.rs` matrix run 1
- Measurement evidence: `evidence/rule.rs` benchmark v1
- Rollback: --disable-optimization ControlFlowCleanup
";
    assert!(record_defects(&repository, "ControlFlowCleanup", "Recommended", valid).is_empty());

    let incomplete = valid
        .replace(
            "Exact rule: ControlFlowCleanup",
            "Exact rule: CopyPropagation",
        )
        .replace(
            "Differential evidence: `corpus/case` corpus run 1",
            "Differential evidence: PENDING",
        );
    let defects = record_defects(
        &repository,
        "ControlFlowCleanup",
        "Recommended",
        &incomplete,
    );
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

    // Completed evidence is unverifiable when it cites no artifact, and a
    // citation that names a moved or imagined artifact rejects.
    let uncited = valid.replace(
        "`evidence/rule.rs::real_test` matrix legs",
        "evidence-matrix legs",
    );
    let defects = record_defects(&repository, "ControlFlowCleanup", "Recommended", &uncited);
    assert!(
        defects.iter().any(|defect| {
            defect.contains(
                "completed `Semantic and corruption evidence:` cites no repository artifact",
            )
        }),
        "missing uncited-evidence defect; saw {defects:?}"
    );

    let phantom = valid.replace("evidence/rule.rs", "evidence/renamed_away.rs");
    let defects = record_defects(&repository, "ControlFlowCleanup", "Recommended", &phantom);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("evidence/renamed_away.rs")),
        "missing phantom-citation defect; saw {defects:?}"
    );

    let phantom_subject = valid.replace("rule.rs::real_test", "rule.rs::renamed_test");
    let defects = record_defects(
        &repository,
        "ControlFlowCleanup",
        "Recommended",
        &phantom_subject,
    );
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("`renamed_test` does not appear")),
        "missing phantom-subject defect; saw {defects:?}"
    );

    let _ = std::fs::remove_dir_all(repository);
}

#[test]
fn staged_record_keeps_schema_while_pending_and_rejects_early_approval() {
    let repository = fixture_repository("staged");
    let staged = "\
# ControlFlowCleanup Promotion
- Exact rule: ControlFlowCleanup
- Approved status: PENDING
- Owner approval: PENDING
- Semantic and corruption evidence: `evidence/rule.rs` evidence-matrix legs
- Differential evidence: PENDING
- Determinism and bounded-work evidence: `evidence/rule.rs::real_test` determinism and budget legs
- Target matrix evidence: PENDING
- Measurement evidence: PENDING
- Rollback: --disable-optimization ControlFlowCleanup
";
    assert!(staged_record_defects(&repository, "ControlFlowCleanup", staged).is_empty());

    let approved_early = staged.replace("Approved status: PENDING", "Approved status: Recommended");
    let defects = staged_record_defects(&repository, "ControlFlowCleanup", &approved_early);
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
    let defects = staged_record_defects(&repository, "ControlFlowCleanup", &wrong_identity);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("Exact rule: ControlFlowCleanup")),
        "missing identity defect; saw {defects:?}"
    );

    let missing_field = staged.replace("- Measurement evidence: PENDING\n", "");
    let defects = staged_record_defects(&repository, "ControlFlowCleanup", &missing_field);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("`Measurement evidence:`")),
        "missing schema-field defect; saw {defects:?}"
    );

    // Staged evidence is still evidence: a completed field needs a citation,
    // and a citation must resolve.
    let uncited = staged.replace("`evidence/rule.rs` evidence", "the evidence");
    let defects = staged_record_defects(&repository, "ControlFlowCleanup", &uncited);
    assert!(
        defects.iter().any(|defect| {
            defect.contains(
                "completed `Semantic and corruption evidence:` cites no repository artifact",
            )
        }),
        "missing staged uncited-evidence defect; saw {defects:?}"
    );

    let phantom = staged.replace("evidence/rule.rs::real_test", "evidence/rule.rs::gone_test");
    let defects = staged_record_defects(&repository, "ControlFlowCleanup", &phantom);
    assert!(
        defects
            .iter()
            .any(|defect| defect.contains("`gone_test` does not appear")),
        "missing staged phantom-subject defect; saw {defects:?}"
    );

    let _ = std::fs::remove_dir_all(repository);
}
