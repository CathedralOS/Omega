use crate::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use provider_planning::plans::SelectedProviderReviewProvenance;
use symbols::SymbolHandle;

/// One consumer-supplied semantic binding after exact selected-plan
/// settlement. The raw row remains visible for persistence/review, while the
/// compiler-resolved declaration symbol prevents later consumers from
/// searching for authority by a readable path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAcceptedSemanticBinding {
    accepted: package_compilation::AcceptedSemanticBinding,
    declaration_symbol: SymbolHandle,
}

impl ResolvedAcceptedSemanticBinding {
    pub const fn accepted(&self) -> &package_compilation::AcceptedSemanticBinding {
        &self.accepted
    }

    pub const fn declaration_symbol(&self) -> SymbolHandle {
        self.declaration_symbol
    }

    pub const fn role(&self) -> package_compilation::AcceptedSemanticBindingRole {
        self.accepted.role()
    }
}

/// Resolve one requirement-only semantic role against an exact package-owned
/// boundary declaration. Unlike Console execution settlement, this does not
/// claim or synthesize a provider plan.
pub fn resolve_accepted_service_binding(
    checked: &CheckedTrees,
    binding: &package_compilation::AcceptedSemanticBinding,
) -> Result<ResolvedAcceptedSemanticBinding, Diagnostic> {
    if !matches!(
        binding.role(),
        package_compilation::AcceptedSemanticBindingRole::FilesystemHostService
            | package_compilation::AcceptedSemanticBindingRole::UefiX64ProgramEntry
    ) || binding.selected_provider_plan_digest().is_some()
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
        .filter_map(|definition| {
            if !definition.is_boundary
                || checked
                    .typed
                    .symbols
                    .symbol_package_identity(definition.symbol)
                    != Some(binding.package())
                || checked.typed.symbols.display_path(definition.symbol, "::")
                    != binding.declaration_path()
            {
                return None;
            }
            let schema = provider_planning::service_schema::from_typed(&checked.typed, definition)?;
            (schema.trait_package_identity == Some(binding.package())
                && package_compilation::accepted_service_schema_digest(binding.role(), &schema)
                    == binding.normalized_schema_digest())
            .then_some((definition.symbol, schema))
        })
        .collect::<Vec<_>>();

    let [(declaration_symbol, schema)] = matches.as_slice() else {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} resolved to {} exact package-owned boundary declarations instead of one",
            binding.role(),
            matches.len(),
        )));
    };
    validate_terminal_authority_permissions(binding, schema)?;
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
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    provenance: &mut [SelectedProviderReviewProvenance],
    selected_target: Option<&str>,
    accepted_console_binding: Option<&package_compilation::AcceptedSemanticBinding>,
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
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
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
                    if identity == effects::CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32
                        && let Some(binding) = accepted_console_binding
                    {
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
        (Some(binding), [declaration_symbol]) => {
            match resolve_terminal_authority_permissions_for_symbol(
                checked,
                binding,
                *declaration_symbol,
            ) {
                Ok(()) => Some(ResolvedAcceptedSemanticBinding {
                    accepted: binding.clone(),
                    declaration_symbol: *declaration_symbol,
                }),
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    None
                }
            }
        }
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

fn resolve_terminal_authority_permissions_for_symbol(
    checked: &CheckedTrees,
    binding: &package_compilation::AcceptedSemanticBinding,
    declaration_symbol: SymbolHandle,
) -> Result<(), Diagnostic> {
    let declarations = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == declaration_symbol)
        .collect::<Vec<_>>();
    let [definition] = declarations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} resolved permission custody to {} boundary declarations instead of one",
            binding.role(),
            declarations.len(),
        )));
    };
    let Some(schema) = provider_planning::service_schema::from_typed(&checked.typed, definition)
    else {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} cannot reconstruct its exact service schema for terminal-authority permission custody",
            binding.role(),
        )));
    };
    validate_terminal_authority_permissions(binding, &schema)
}

fn validate_terminal_authority_permissions(
    binding: &package_compilation::AcceptedSemanticBinding,
    schema: &effects::provider_plan::ServiceSchema,
) -> Result<(), Diagnostic> {
    for permission in binding.terminal_authority_permissions() {
        let matches = schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == permission.requirement_identity())
            .count();
        if matches != 1 {
            return Err(Diagnostic::error(format!(
                "accepted semantic binding {:?} terminal-authority permission `{}` rejoins {matches} exact service methods instead of one",
                binding.role(),
                permission.requirement_identity(),
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::{
        ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
        provider_plan::{ServiceMethod, ServiceSchema},
    };

    fn package() -> semantic_vocabulary::PackageKeyIdentity {
        semantic_vocabulary::PackageKeyIdentity::from_digest([73; 32])
            .expect("nonzero package identity")
    }

    fn schema() -> ServiceSchema {
        ServiceSchema {
            trait_name: "FilesystemHost".into(),
            trait_package_identity: Some(package()),
            methods: vec![
                ServiceMethod {
                    name: "read".into(),
                    requirement_owner: "FilesystemHost".into(),
                    requirement_owner_package_identity: Some(package()),
                    requirement_identity: "FilesystemHost::read#exact".into(),
                    ..ServiceMethod::default()
                },
                ServiceMethod {
                    name: "write".into(),
                    requirement_owner: "FilesystemHost".into(),
                    requirement_owner_package_identity: Some(package()),
                    requirement_identity: "FilesystemHost::write#exact".into(),
                    ..ServiceMethod::default()
                },
            ],
        }
    }

    fn binding(requirement_identity: &str) -> package_compilation::AcceptedSemanticBinding {
        let schema = schema();
        let schema_digest = schema.identity_digest();
        package_compilation::AcceptedSemanticBinding::new_service(
            package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
            package(),
            "FilesystemHost",
            schema_digest,
        )
        .unwrap()
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            schema_digest,
            requirement_identity,
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::FilesystemContentRead,
            ]),
        )])
        .unwrap()
    }

    #[test]
    fn exact_partial_permission_set_rejoins_reconstructed_schema() {
        validate_terminal_authority_permissions(&binding("FilesystemHost::read#exact"), &schema())
            .expect("one explicit row may cover a strict subset of schema methods");
    }

    #[test]
    fn substituted_requirement_identity_rejects_at_resolution() {
        let diagnostic = validate_terminal_authority_permissions(
            &binding("FilesystemHost::rename#substituted"),
            &schema(),
        )
        .expect_err("permission must rejoin an exact reconstructed schema method");
        assert!(
            diagnostic
                .message
                .contains("rejoins 0 exact service methods")
        );
    }
}
