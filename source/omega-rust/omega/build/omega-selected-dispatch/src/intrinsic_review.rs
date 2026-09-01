use crate::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding,
};
use omega_provider_planning::plans::SelectedProviderReviewProvenance;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

/// One consumer-supplied semantic binding after exact selected-plan
/// settlement. The raw row remains visible for persistence/review, while the
/// compiler-resolved declaration symbol prevents later consumers from
/// searching for authority by a readable path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAcceptedSemanticBinding {
    accepted: omega_package_compilation::AcceptedSemanticBinding,
    declaration_symbol: SymbolHandle,
}

impl ResolvedAcceptedSemanticBinding {
    pub const fn accepted(&self) -> &omega_package_compilation::AcceptedSemanticBinding {
        &self.accepted
    }

    pub const fn declaration_symbol(&self) -> SymbolHandle {
        self.declaration_symbol
    }

    pub const fn role(&self) -> omega_package_compilation::AcceptedSemanticBindingRole {
        self.accepted.role()
    }
}

/// Resolve one requirement-only semantic role against an exact package-owned
/// boundary declaration. Unlike Console execution settlement, this does not
/// claim or synthesize a provider plan.
pub fn resolve_accepted_service_binding(
    checked: &CheckedTrees,
    binding: &omega_package_compilation::AcceptedSemanticBinding,
) -> Result<ResolvedAcceptedSemanticBinding, Diagnostic> {
    if binding.role()
        != omega_package_compilation::AcceptedSemanticBindingRole::FilesystemHostService
        || binding.selected_provider_plan_digest().is_some()
    {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} is not a requirement-only service binding",
            binding.role(),
        )));
    }

    let matches = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && checked
                    .typed
                    .symbols
                    .symbol_package_identity(definition.symbol)
                    == Some(binding.package())
                && checked.typed.symbols.display_path(definition.symbol, "::")
                    == binding.declaration_path()
                && omega_effects::provider_plan::ServiceSchema::from_typed(
                    &checked.typed,
                    definition,
                )
                .is_some_and(|schema| {
                    schema.trait_package_identity == Some(binding.package())
                        && schema.identity_digest() == binding.normalized_schema_digest()
                })
        })
        .map(|definition| definition.symbol)
        .collect::<Vec<_>>();

    let [declaration_symbol] = matches.as_slice() else {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} resolved to {} exact package-owned boundary declarations instead of one",
            binding.role(),
            matches.len(),
        )));
    };
    Ok(ResolvedAcceptedSemanticBinding {
        accepted: binding.clone(),
        declaration_symbol: *declaration_symbol,
    })
}

/// Close row-aligned package-review identity after checked provider execution
/// has been selected.
///
/// The retained sidecar is deliberately separate from the authored
/// realization-machine declaration. Non-intrinsic rows retain `None`.
/// Unsupported compiler-intrinsic executions also retain `None`; package
/// review rederives that unsupported child and rejects it until a closed
/// identity exists.
pub fn retain_selected_compiler_intrinsic_review_identities(
    checked: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    provenance: &mut [SelectedProviderReviewProvenance],
    selected_target: Option<&str>,
    accepted_console_binding: Option<&omega_package_compilation::AcceptedSemanticBinding>,
) -> Result<Option<ResolvedAcceptedSemanticBinding>, Vec<Diagnostic>> {
    let plans = selected_provider_plans.plans();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-owned review provenance",
        )]);
    }

    let mut retained_rows = Vec::with_capacity(plans.len());
    let mut accepted_matches = Vec::new();
    let mut diagnostics = Vec::new();
    for (plan, retained) in plans.iter().zip(provenance.iter()) {
        if retained.plan != *plan
            || retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
            || !retained.row_compiler_intrinsic_executions.is_empty()
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete, misaligned, or already-populated compiler-intrinsic review state",
                plan.name,
            )));
            retained_rows.push(Vec::new());
            continue;
        }

        let mut rows = Vec::with_capacity(plan.rows.len());
        for ((row, requirement_symbol), realization_symbol) in plan
            .rows
            .iter()
            .zip(&retained.provider.row_requirements)
            .zip(&retained.provider.row_realizations)
        {
            if !matches!(
                row.binding,
                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                rows.push(None);
                continue;
            }
            match derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding(
                checked,
                plan,
                retained.provider.schema,
                row,
                *requirement_symbol,
                *realization_symbol,
                selected_target,
                accepted_console_binding,
            ) {
                Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(identity))) => {
                    if accepted_console_binding.is_some_and(|binding| {
                        crate::compiler_intrinsic::accepted_binding_matches_selected_row_identity(
                            checked,
                            plan,
                            retained.provider.schema.symbol(),
                            *requirement_symbol,
                            *realization_symbol,
                            binding,
                        )
                    }) {
                        accepted_matches.push(retained.provider.schema.symbol());
                    }
                    rows.push(Some(identity))
                }
                Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported)) => {
                    if let Some(binding) = accepted_console_binding {
                        match crate::compiler_intrinsic::accepted_binding_matches_console_exit_process_i32_row(
                            checked,
                            plan,
                            row,
                            retained.provider.schema.symbol(),
                            *requirement_symbol,
                            *realization_symbol,
                            binding,
                        ) {
                            Ok(true) => accepted_matches.push(retained.provider.schema.symbol()),
                            Ok(false) => {}
                            Err(diagnostic) => diagnostics.push(diagnostic),
                        }
                    }
                    rows.push(None)
                }
                Ok(None) => rows.push(None),
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    rows.push(None);
                }
            }
        }
        retained_rows.push(rows);
    }

    let resolved = match (accepted_console_binding, accepted_matches.as_slice()) {
        (None, _) => None,
        (Some(binding), [declaration_symbol]) => Some(ResolvedAcceptedSemanticBinding {
            accepted: binding.clone(),
            declaration_symbol: *declaration_symbol,
        }),
        (Some(binding), []) => {
            diagnostics.push(Diagnostic::error(format!(
                "accepted semantic binding {:?} was not consumed by one exact selected provider plan",
                binding.role(),
            )));
            None
        }
        (Some(binding), matches) => {
            diagnostics.push(Diagnostic::error(format!(
                "accepted semantic binding {:?} ambiguously matched {} selected provider rows",
                binding.role(),
                matches.len(),
            )));
            None
        }
    };

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (retained, rows) in provenance.iter_mut().zip(retained_rows) {
        retained.row_compiler_intrinsic_executions = rows;
    }
    Ok(resolved)
}
