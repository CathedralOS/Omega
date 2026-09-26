use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(super) fn validate_review_compilation(
    compilation: &PackageReviewInput<'_>,
) -> Result<(PackageKeyIdentity, TargetProfile), Vec<Diagnostic>> {
    let package = compilation.custody.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    let target = compilation
        .custody
        .selected_target_profile()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "package review requires one explicit target selection",
            )]
        })?;
    compilation
        .custody
        .evaluated_via_bindings()
        .validate_against_typed(&compilation.typed)?;
    if compilation.custody.evaluated_via_bindings().target() != Some(target) {
        return Err(vec![Diagnostic::error(
            "package review target disagrees with the evaluated `via` binding table",
        )]);
    }
    let derived_operator_realizations =
        typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &compilation.typed,
        );
    if derived_operator_realizations != compilation.facts.operators.operator_realization_contracts {
        return Err(vec![Diagnostic::error(format!(
            "retained checked operator-realization contracts do not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation
                .facts
                .operators
                .operator_realization_contracts
                .len(),
            derived_operator_realizations.len(),
        ))]);
    }
    let derived_stand_downs =
        validation::collect_contract_entailment_stand_downs(&compilation.typed);
    if derived_stand_downs != compilation.custody.contract_entailment_stand_downs() {
        return Err(vec![Diagnostic::error(format!(
            "retained contract-entailment stand-downs do not equal fresh compiler rederivation (retained {} rows, derived {} rows)",
            compilation.custody.contract_entailment_stand_downs().len(),
            derived_stand_downs.len(),
        ))]);
    }

    Ok((package, target))
}
