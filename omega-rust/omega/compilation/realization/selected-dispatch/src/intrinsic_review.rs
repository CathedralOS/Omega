//! Review of accepted compiler-intrinsic bindings: resolving the accepted
//! service binding and retaining the selected review identities.

use crate::SelectedCompilerIntrinsicExecutionIdentity;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use provider_planning::SelectedProviderReviewProvenance;
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
pub(crate) fn resolve_accepted_service_binding(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    binding: &package_compilation::AcceptedSemanticBinding,
) -> Result<ResolvedAcceptedSemanticBinding, Diagnostic> {
    if !matches!(
        binding.role(),
        package_compilation::AcceptedSemanticBindingRole::FilesystemHostService
            | package_compilation::AcceptedSemanticBindingRole::TimeHostService
            | package_compilation::AcceptedSemanticBindingRole::UefiX64ProgramEntry
            | package_compilation::AcceptedSemanticBindingRole::MacosArm64ProgramEntry
            | package_compilation::AcceptedSemanticBindingRole::MacosX64ProgramEntry
            | package_compilation::AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry
            | package_compilation::AcceptedSemanticBindingRole::LinuxArm64ProgramEntry
            | package_compilation::AcceptedSemanticBindingRole::WindowsX64ProgramEntry
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
                || checked.symbols.display_path(definition.symbol, "::")
                    != binding.declaration_path()
            {
                return None;
            }
            let schema = provider_planning::service_schema::from_typed(
                provider_planning::ProviderPlanningContext::new(
                    checked,
                    source::DependencyScope::Product,
                ),
                calling_plans,
                definition,
            )?;
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
pub(crate) fn retain_selected_compiler_intrinsic_review_identities(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    provenance: &mut [SelectedProviderReviewProvenance],
    selected_target: Option<&str>,
    accepted_console_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_process_exit_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<Vec<ResolvedAcceptedSemanticBinding>, Vec<Diagnostic>> {
    let plans = selected_provider_plans.plans();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-owned review provenance",
        )]);
    }

    let mut retained_rows = Vec::with_capacity(plans.len());
    let mut console_accepted_matches = Vec::new();
    let mut process_exit_accepted_matches = Vec::new();
    let mut diagnostics = Vec::new();
    for (plan, retained) in plans.iter().zip(provenance.iter()) {
        if retained.plan != *plan
            || retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
            || retained.provider.row_placed_accessors.len() != plan.rows.len()
            || !retained.row_compiler_intrinsic_executions.is_empty()
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete, misaligned, or already-populated compiler-intrinsic review state",
                plan.name,
            )));
            retained_rows.push(Vec::new());
            continue;
        }

        let catalog_context = provider_planning::ProviderPlanningContext::new(
            checked,
            source::DependencyScope::Product,
        );
        let expected_placed =
            match provider_planning::SealedPlacedAccessor::rejoin_plan(catalog_context, plan) {
                Ok(witness) => witness,
                Err(error) => {
                    diagnostics.push(error);
                    retained_rows.push(Vec::new());
                    continue;
                }
            };
        let catalog_role_matches = match retained.provider.schema {
            provider_planning::ProviderSchemaDeclaration::SealedPlacedAccessor(symbol) => {
                expected_placed
                    .as_ref()
                    .is_some_and(|witness| witness.machine() == symbol)
                    && retained.provider.row_placed_accessors.as_slice()
                        == std::slice::from_ref(&expected_placed)
            }
            _ => {
                expected_placed.is_none()
                    && retained
                        .provider
                        .row_placed_accessors
                        .iter()
                        .all(Option::is_none)
            }
        };
        if !catalog_role_matches {
            diagnostics.push(Diagnostic::error(
                "selected intrinsic review substituted its placed catalog custody",
            ));
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
                // A hosted exit realized through the target's DLL linkage
                // (Windows: kernel32 `ExitProcess`) consumes the accepted
                // Console or ProcessExit binding exactly as an intrinsic
                // exit row does; the binding names the requirement and the
                // provider row, not the realization behind them.
                accumulate_accepted_row_match(
                    checked,
                    plan,
                    row,
                    retained.provider.schema.symbol(),
                    *requirement_symbol,
                    *realization_symbol,
                    accepted_console_binding,
                    accepted_process_exit_binding,
                    &mut console_accepted_matches,
                    &mut process_exit_accepted_matches,
                    &mut diagnostics,
                );
                rows.push(None);
                continue;
            }
            let unresolved = |accepted| crate::compiler_intrinsic::AcceptedRowBinding {
                accepted,
                declaration_symbol: None,
            };
            match crate::compiler_intrinsic::row_execution_identity(
                checked,
                plan,
                retained.provider.schema,
                row,
                *requirement_symbol,
                *realization_symbol,
                selected_target,
                accepted_console_binding.map(unresolved),
                accepted_process_exit_binding.map(unresolved),
            ) {
                Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(identity))) => {
                    if identity == effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
                    {
                        accumulate_accepted_row_match(
                            checked,
                            plan,
                            row,
                            retained.provider.schema.symbol(),
                            *requirement_symbol,
                            *realization_symbol,
                            accepted_console_binding,
                            accepted_process_exit_binding,
                            &mut console_accepted_matches,
                            &mut process_exit_accepted_matches,
                            &mut diagnostics,
                        );
                    }
                    rows.push(Some(identity))
                }
                Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Unsupported)) => {
                    accumulate_accepted_row_match(
                        checked,
                        plan,
                        row,
                        retained.provider.schema.symbol(),
                        *requirement_symbol,
                        *realization_symbol,
                        accepted_console_binding,
                        accepted_process_exit_binding,
                        &mut console_accepted_matches,
                        &mut process_exit_accepted_matches,
                        &mut diagnostics,
                    );
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

    let mut resolved = Vec::new();
    for (binding, matches) in [
        (accepted_console_binding, console_accepted_matches),
        (accepted_process_exit_binding, process_exit_accepted_matches),
    ] {
        match (binding, matches.as_slice()) {
            (None, _) => {}
            (Some(binding), [declaration_symbol]) => {
                match resolve_terminal_authority_permissions_for_symbol(
                    checked,
                    calling_plans,
                    binding,
                    *declaration_symbol,
                ) {
                    Ok(()) => resolved.push(ResolvedAcceptedSemanticBinding {
                        accepted: binding.clone(),
                        declaration_symbol: *declaration_symbol,
                    }),
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            (Some(binding), []) => {
                diagnostics.push(Diagnostic::error(format!(
                    "accepted semantic binding {:?} was not consumed by one exact selected provider plan",
                    binding.role(),
                )));
            }
            (Some(binding), matches) => {
                diagnostics.push(Diagnostic::error(format!(
                    "accepted semantic binding {:?} ambiguously matched {} selected provider rows",
                    binding.role(),
                    matches.len(),
                )));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (retained, rows) in provenance.iter_mut().zip(retained_rows) {
        retained.row_compiler_intrinsic_executions = rows;
    }
    Ok(resolved)
}

/// Consume one hosted-exit row against each supplied consumer binding. The
/// Console and canonical ProcessExit roles are independent: a row can consume
/// at most one of them, and each binding must be consumed by exactly one
/// declaration below.
#[allow(clippy::too_many_arguments)]
fn accumulate_accepted_row_match(
    checked: &CheckedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    schema_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    accepted_console_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_process_exit_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    console_matches: &mut Vec<SymbolHandle>,
    process_exit_matches: &mut Vec<SymbolHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(binding) = accepted_console_binding {
        match crate::compiler_intrinsic::accepted_binding_matches_exit_process_row(
            crate::compiler_intrinsic::HostedService::Console,
            checked,
            plan,
            row,
            schema_symbol,
            requirement_symbol,
            realization_symbol,
            binding,
        ) {
            Ok(true) => console_matches.push(schema_symbol),
            Ok(false) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if let Some(binding) = accepted_process_exit_binding {
        match crate::compiler_intrinsic::accepted_binding_matches_exit_process_row(
            crate::compiler_intrinsic::HostedService::ProcessExit,
            checked,
            plan,
            row,
            schema_symbol,
            requirement_symbol,
            realization_symbol,
            binding,
        ) {
            Ok(true) => process_exit_matches.push(schema_symbol),
            Ok(false) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
}

fn resolve_terminal_authority_permissions_for_symbol(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
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
    let Some(schema) = provider_planning::service_schema::from_typed(
        provider_planning::ProviderPlanningContext::new(checked, source::DependencyScope::Product),
        calling_plans,
        definition,
    ) else {
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
        let requirement_identity = binding
            .terminal_authority_permission_identities()
            .text(permission.requirement_identity())
            .unwrap_or_default();
        let matches = schema
            .methods
            .iter()
            .filter(|method| {
                schema.identity_text(method.requirement_identity) == requirement_identity
            })
            .count();
        if matches != 1 {
            return Err(Diagnostic::error(format!(
                "accepted semantic binding {:?} terminal-authority permission `{requirement_identity}` rejoins {matches} exact service methods instead of one",
                binding.role(),
            )));
        }
    }
    Ok(())
}
