use omega_compiler::CheckedCompilation;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(super) fn validate_review_compilation(
    compilation: &CheckedCompilation,
) -> Result<(PackageKeyIdentity, TargetProfile), Vec<Diagnostic>> {
    let package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    let target = compilation.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires one explicit target selection",
        )]
    })?;
    compilation
        .evaluated_via_bindings()
        .validate_against_typed(&compilation.typed)?;
    if compilation.evaluated_via_bindings().target() != Some(target) {
        return Err(vec![Diagnostic::error(
            "package review target disagrees with the evaluated `via` binding table",
        )]);
    }
    let package_stand_downs = compilation
        .contract_entailment_stand_downs()
        .iter()
        .filter(|stand_down| {
            compilation
                .symbols
                .symbol_package_identity(stand_down.machine_symbol)
                == Some(package)
        })
        .collect::<Vec<_>>();
    if !package_stand_downs.is_empty() {
        return Err(package_stand_downs
            .into_iter()
            .map(|stand_down| {
                Diagnostic::error(format!(
                    "package review rejects unresolved contract-entailment stand-down in machine `{}` (symbol {}), contract {}, fact {}: {}",
                    compilation
                        .symbols
                        .display_path(stand_down.machine_symbol, "::"),
                    stand_down.machine_symbol.arena_index(),
                    stand_down.contract_index,
                    stand_down.fact_index,
                    stand_down.reason.label(),
                ))
            })
            .collect());
    }

    let derived_operator_realizations =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
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

    Ok((package, target))
}
