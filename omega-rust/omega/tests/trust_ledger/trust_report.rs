//! Legacy standalone trust-report coverage. Only exact accepted-machine and
//! selected-provider grants may create receipts; domains and unmatched strings
//! are not trust subjects.
//!
//! Fixtures shared by the trust report tests: compilation and the ranked
//! receipt helpers.

#[path = "trust_report/locks_grants_and_receipts.rs"]
mod locks_grants_and_receipts;
#[path = "trust_report/plans_and_provider_rows.rs"]
mod plans_and_provider_rows;

use omega::compiler::CompileOptions;

fn compile(
    options: CompileOptions,
) -> Result<omega::compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    let admissions = omega::trust_ledger::read_trust_admissions(&options.root_path)?;
    let root_path = options.root_path.clone();
    let report = omega::compiler::compile(
        omega::compiler::CompileRequest::new(options).with_accepted_trust_admissions(admissions),
    )
    .and_then(omega::compiler::CompileOutcomes::into_single_report)?;
    let settlement = report.trust_admission_settlement();
    if settlement.is_exactly_admitted() {
        assert!(!report.wrote_output());
        return Ok(report);
    }
    if !root_path
        .parent()
        .is_some_and(|project| project.join("omega.admissions").exists())
    {
        omega::trust_ledger::accept_trust_admissions(&root_path, settlement.required())?;
        assert!(!report.wrote_output());
        return Ok(report);
    }
    let added = settlement
        .unresolved()
        .iter()
        .filter(|required| {
            !settlement
                .unused()
                .iter()
                .any(|accepted| accepted.commitment() == required.commitment())
        })
        .map(|row| row.commitment().to_owned())
        .collect::<Vec<_>>();
    let removed = settlement
        .unused()
        .iter()
        .filter(|accepted| {
            !settlement
                .unresolved()
                .iter()
                .any(|required| required.commitment() == accepted.commitment())
        })
        .map(|row| row.commitment().to_owned())
        .collect::<Vec<_>>();
    let changed = settlement
        .unresolved()
        .iter()
        .filter_map(|required| {
            settlement
                .unused()
                .iter()
                .find(|accepted| accepted.commitment() == required.commitment())
                .map(|accepted| {
                    format!(
                        "{} ({} -> {})",
                        required.commitment(),
                        accepted.digest(),
                        required.digest()
                    )
                })
        })
        .collect::<Vec<_>>();
    let display = |rows: &[String]| {
        if rows.is_empty() {
            "none".to_owned()
        } else {
            rows.join(", ")
        }
    };
    Err(vec![diagnostics::Diagnostic::error(format!(
        "granted statement drifted: the complete trust receipt set no longer matches omega.admissions -- added: {}; removed: {}; changed: {}",
        display(&added),
        display(&removed),
        display(&changed),
    ))])
}
