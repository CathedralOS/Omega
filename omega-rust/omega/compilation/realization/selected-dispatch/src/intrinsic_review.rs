// Review of accepted compiler-intrinsic bindings: resolving the accepted
// service binding, retaining selected review identities, and producing direct
// boundary leaves.
//
// ResolvedAcceptedSemanticBinding pairs one consumer-supplied semantic binding
// with its exact ServiceDeclaration record after selected-plan settlement so
// later consumers cannot search for authority by readable path or retain a
// source-arena coordinate.
//
// retain_selected_compiler_intrinsic_review_identities closes row-aligned
// package-review identity after checked provider execution is selected.
// Non-intrinsic and unsupported compiler-intrinsic rows retain None so package
// review rederives and rejects unsupported children until a closed identity
// exists. Neither a Build locator nor a settlement table supplies a sealed
// intrinsic execution identity or an authored completion. Console and
// ProcessExit roles are independent: a row consumes at most one of them, and a
// hosted exit realized through target DLL linkage consumes the accepted
// Console or ProcessExit binding just as an intrinsic exit row does.
//
// produce_direct_boundary_leaves constructs complete target-call supplies
// after host bindings settle, borrowing original source rows from planning and
// closing every original row before coalescing equal supplies.

use crate::SelectedCompilerIntrinsicExecutionIdentity;
use crate::compiler_intrinsic::{AcceptedRowBinding, HostedService};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use provider_planning::SelectedProviderReviewProvenance;
use provider_planning::service_declarations::{ServiceDeclaration, ServiceDeclarations};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAcceptedSemanticBinding {
    accepted: package_compilation::AcceptedSemanticBinding,
    declaration: Arc<ServiceDeclaration>,
}

impl ResolvedAcceptedSemanticBinding {
    pub const fn accepted(&self) -> &package_compilation::AcceptedSemanticBinding {
        &self.accepted
    }

    pub fn declaration(&self) -> &ServiceDeclaration {
        &self.declaration
    }

    pub const fn role(&self) -> package_compilation::AcceptedSemanticBindingRole {
        self.accepted.role()
    }
}

pub(crate) fn resolve_accepted_service_binding(
    services: &ServiceDeclarations,
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

    let declaration = services
        .select(binding.package(), binding.declaration_path())?
        .declaration;
    if declaration.schema.declaration.trait_package_identity != Some(binding.package())
        || package_compilation::accepted_service_schema_digest(binding.role(), &declaration.schema)
            != binding.normalized_schema_digest()
    {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} changed its exact owner or service schema",
            binding.role(),
        )));
    }
    validate_terminal_authority_permissions(binding, &declaration.schema)?;
    Ok(ResolvedAcceptedSemanticBinding {
        accepted: binding.clone(),
        declaration: Arc::clone(declaration),
    })
}

pub(crate) fn retain_selected_compiler_intrinsic_review_identities(
    candidates: &[provider_planning::ProviderPlanCandidate],
    external_providers: &provider_planning::ExternalProviderDeclarations,
    services: &ServiceDeclarations,
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
            || retained.provider.row_accessors.len() != plan.rows.len()
            || retained.row_external_completion_leaves.len() != plan.rows.len()
            || !retained.row_compiler_intrinsic_executions.is_empty()
        {
            diagnostics.push(Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete, misaligned, or already-populated compiler-intrinsic review state",
                plan.name,
            )));
            retained_rows.push(Vec::new());
            continue;
        }

        let mut candidates = candidates
            .iter()
            .filter(|candidate| candidate.matches_selection(retained));
        let candidate = match (candidates.next(), candidates.next()) {
            (Some(candidate), None) => candidate,
            _ => {
                diagnostics.push(Diagnostic::error(
                    "selected intrinsic review has no unique original provider candidate",
                ));
                retained_rows.push(Vec::new());
                continue;
            }
        };
        let expected_access = match candidate.accessor() {
            Ok(witness) => witness,
            Err(error) => {
                diagnostics.push(error);
                retained_rows.push(Vec::new());
                continue;
            }
        };
        let catalog_role_matches = match retained.provider.schema {
            provider_planning::ProviderSchemaDeclaration::SealedAccessor(symbol) => {
                expected_access
                    .as_ref()
                    .is_some_and(|witness| witness.machine() == symbol)
                    && retained.provider.row_accessors.as_slice()
                        == std::slice::from_ref(&expected_access.cloned())
            }
            _ => {
                expected_access.is_none()
                    && retained.provider.row_accessors.iter().all(Option::is_none)
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
        for (row_index, (((row, requirement_symbol), realization_symbol), completion_leaves)) in
            plan.rows
                .iter()
                .zip(&retained.provider.row_requirements)
                .zip(&retained.provider.row_realizations)
                .zip(&retained.row_external_completion_leaves)
                .enumerate()
        {
            let Some(realization_symbol) = realization_symbol.authored() else {
                if matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
                    diagnostics.push(Diagnostic::error(
                        "intrinsic row has no exact authored realization",
                    ));
                }
                rows.push(None);
                continue;
            };
            let Some(declarations) = candidate.row_declarations(row_index) else {
                diagnostics.push(Diagnostic::error(
                    "selected intrinsic review lost its prepared original-row declarations",
                ));
                rows.push(None);
                continue;
            };
            let exit_process = &declarations.exit_process;
            let mut accumulate = |diagnostics: &mut Vec<Diagnostic>| {
                let checked = !completion_leaves.is_empty();
                let schema_symbol = retained.provider.schema.symbol();
                if let Some(binding) = accepted_console_binding {
                    match exit_process.matches(HostedService::Console, plan, checked, binding) {
                        Ok(true) => console_accepted_matches.push(schema_symbol),
                        Ok(false) => {}
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }
                }
                if let Some(binding) = accepted_process_exit_binding {
                    match exit_process.matches(HostedService::ProcessExit, plan, checked, binding) {
                        Ok(true) => process_exit_accepted_matches.push(schema_symbol),
                        Ok(false) => {}
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }
                }
            };
            if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
                accumulate(&mut diagnostics);
                rows.push(None);
                continue;
            }
            let unresolved = |accepted| AcceptedRowBinding {
                accepted,
                declaration_identity: None,
                public_plan: None,
            };
            let Some(intrinsic) = declarations.intrinsic.as_ref() else {
                diagnostics.push(Diagnostic::error(
                    "selected intrinsic review has no prepared intrinsic declaration",
                ));
                rows.push(None);
                continue;
            };
            let numeric = crate::compiler_intrinsic::numeric_execution(
                crate::NumericProviderSelection {
                    candidate: external_providers,
                    custody: external_providers,
                    provider: retained.provider.provider_type,
                },
                plan,
                retained.provider.schema,
                row,
                *requirement_symbol,
                realization_symbol,
            );
            let execution = numeric.and_then(|numeric| match numeric {
                Some(identity) => Ok(SelectedCompilerIntrinsicExecutionIdentity::Closed(identity)),
                None => crate::compiler_intrinsic::settle_declaration(
                    intrinsic,
                    plan,
                    selected_target,
                    accepted_console_binding.map(unresolved),
                    accepted_process_exit_binding.map(unresolved),
                ),
            });
            match execution {
                Ok(SelectedCompilerIntrinsicExecutionIdentity::Closed(identity)) => {
                    if identity == effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
                    {
                        accumulate(&mut diagnostics);
                    }
                    rows.push((!identity.is_numeric()).then_some(identity))
                }
                Ok(SelectedCompilerIntrinsicExecutionIdentity::Unsupported) => {
                    accumulate(&mut diagnostics);
                    rows.push(None)
                }
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
                match services
                    .for_symbol(*declaration_symbol)
                    .and_then(|service| {
                        validate_terminal_authority_permissions(
                            binding,
                            &service.declaration.schema,
                        )?;
                        Ok(Arc::clone(service.declaration))
                    }) {
                    Ok(declaration) => resolved.push(ResolvedAcceptedSemanticBinding {
                        accepted: binding.clone(),
                        declaration,
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

pub(crate) fn produce_direct_boundary_leaves(
    external_providers: &provider_planning::ExternalProviderDeclarations,
    selected: &[SelectedProviderReviewProvenance],
    candidates: &[provider_planning::ProviderPlanCandidate],
    services: &ServiceDeclarations,
    target: Option<&str>,
    bindings: &[ResolvedAcceptedSemanticBinding],
) -> Result<Vec<backend_plan::DirectBoundaryLeaf>, Vec<Diagnostic>> {
    let mut leaves = Vec::new();
    for candidate in candidates {
        for input in candidate.direct_boundaries() {
            let execution = match &input.mechanism {
                provider_planning::DirectBoundaryMechanism::Foreign(binding) => {
                    backend_plan::DirectBoundaryExecution::Foreign(binding.clone())
                }
                provider_planning::DirectBoundaryMechanism::Intrinsic => {
                    let declaration = candidate
                        .direct_declarations(input)
                        .map_err(|diagnostic| vec![diagnostic])?
                        .intrinsic
                        .as_ref()
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "direct intrinsic has no prepared original-row declaration",
                            )]
                        })?;
                    let source = candidate
                        .direct_source(input)
                        .map_err(|diagnostic| vec![diagnostic])?;
                    let accepted = |role| -> Result<Option<AcceptedRowBinding<'_>>, Diagnostic> {
                        let Some(binding) = bindings.iter().find(|binding| binding.role() == role)
                        else {
                            return Ok(None);
                        };
                        let declaration_symbol = services.rejoin(binding.declaration())?.symbol;
                        let Some(public) = selected.iter().find(|provider| {
                            provider.provider.schema.symbol() == declaration_symbol
                                && Some(provider.plan.identity_digest())
                                    == binding.accepted().selected_provider_plan_digest()
                        }) else {
                            return Ok(None);
                        };
                        Ok(Some(AcceptedRowBinding {
                            accepted: binding.accepted(),
                            declaration_identity: Some(
                                binding.declaration().nominal_identity.as_str(),
                            ),
                            public_plan: Some(&public.plan),
                        }))
                    };
                    let numeric = crate::compiler_intrinsic::numeric_execution(
                        crate::NumericProviderSelection {
                            candidate: external_providers,
                            custody: external_providers,
                            provider: source.provider,
                        },
                        source.plan,
                        source.schema,
                        source.row,
                        source.requirement,
                        source.realization,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    let closed = match numeric {
                        Some(identity) => Some(identity),
                        None => declaration.settle(
                            source.plan, target,
                            accepted(package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32).map_err(|diagnostic| vec![diagnostic])?,
                            accepted(package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32).map_err(|diagnostic| vec![diagnostic])?,
                        ).map_err(|diagnostic| vec![diagnostic])?,
                    };
                    backend_plan::DirectBoundaryExecution::Intrinsic(closed)
                }
            };
            let leaf = input
                .finish(candidate.plan(), execution)
                .map_err(|diagnostic| vec![diagnostic])?;
            if !leaves.contains(&leaf) {
                leaves.push(leaf);
            }
        }
    }
    Ok(leaves)
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
            .declaration
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
