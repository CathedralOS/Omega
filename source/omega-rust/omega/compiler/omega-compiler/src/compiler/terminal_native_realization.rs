//! Native re-entry for a retained Terminal product with source-evaluated imports.
//!
//! This coordinator owns the exact-plan join. Callers supply only independently
//! admitted execution and same-stack evidence; they cannot select a provider
//! plan, locator, target, compiler builtin, or checked-scope receipt.

use omega_installation_evidence::ProviderExecutionEvidence;
use omega_task_plans::AdmittedSameStackContribution;
use psi_diagnostics::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};

/// Externally admitted custody for one demanded source-evaluated import.
///
/// Construction does not admit either input. Native realization independently
/// rejoins both values to the exact retained provider plan and import row.
#[derive(Debug, Clone, Copy)]
pub struct SourceEvaluatedImportSettlement<'evidence> {
    provider_execution: &'evidence dyn ProviderExecutionEvidence,
    same_stack: &'evidence AdmittedSameStackContribution,
}

impl<'evidence> SourceEvaluatedImportSettlement<'evidence> {
    pub const fn new(
        provider_execution: &'evidence dyn ProviderExecutionEvidence,
        same_stack: &'evidence AdmittedSameStackContribution,
    ) -> Self {
        Self {
            provider_execution,
            same_stack,
        }
    }

    pub const fn provider_execution(self) -> &'evidence dyn ProviderExecutionEvidence {
        self.provider_execution
    }

    pub const fn same_stack(self) -> &'evidence AdmittedSameStackContribution {
        self.same_stack
    }
}

/// Consume one retained Terminal product and realize its demanded evaluated
/// imports through externally admitted execution and same-stack custody.
///
/// Direct callback custody is normalized from the exact retained placement and
/// occurrence pair. The native pipeline currently consumes that row through
/// physical assignment and then rejects at the explicit emission fence.
pub fn realize_retained_terminal_artifact_with_source_evaluated_imports(
    retained: omega_compilation_report::RetainedTerminalArtifact,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    imports: &[SourceEvaluatedImportSettlement<'_>],
) -> Result<omega_compilation_report::RetainedNativeArtifact, Vec<Diagnostic>> {
    let accepted_package_permissions =
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy();
    let receiving_permissions =
        omega_terminal_psi_to_native_artifact::current_terminal_authority_permission_policy();
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        profile,
        optimization_selections,
        omega_terminal_psi_to_native_artifact::current_terminal_authority_policy(),
        accepted_package_permissions,
        receiving_permissions,
        imports,
    )
}

/// Realize a retained Terminal product under independently supplied package
/// acceptance and receiving-authority policies.
///
/// The package policy must be the exact canonical projection from root-policy
/// accepted evidence and must equal the rows preserved in the retained
/// proposal. The receiving policy may contain unrelated rows but may neither
/// omit nor alter any accepted package row. Normalized foreign imports still
/// require explicit physical-policy rows. The compatibility entrypoint above
/// supplies empty package and receiving policies and therefore remains
/// deny-by-absence for every retained package permission.
///
/// This is the package-agnostic lower-level seam: passing freely constructed
/// policy data here does not itself establish package admission. Package
/// orchestration must consume opaque accepted evidence, bind it to the
/// retained report's production subject, and derive this exact package policy
/// before invoking the seam.
pub fn realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
    retained: omega_compilation_report::RetainedTerminalArtifact,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    terminal_authority_policy: omega_terminal_psi_to_native_artifact::TerminalAuthorityPolicy,
    accepted_package_terminal_authority_permission_policy:
        omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy,
    terminal_authority_permission_policy:
        omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy,
    imports: &[SourceEvaluatedImportSettlement<'_>],
) -> Result<omega_compilation_report::RetainedNativeArtifact, Vec<Diagnostic>> {
    retained
        .validate()
        .map_err(|message| diagnostic("retained Terminal product", message))?;
    let (artifact, callback_placements, proposal) = retained.into_parts();
    let proposal = proposal.ok_or_else(|| {
        diagnostic(
            "retained Terminal product",
            "source-evaluated import realization requires one native proposal",
        )
    })?;
    proposal
        .validate_for_artifact(&artifact)
        .map_err(|message| diagnostic("Terminal native proposal", message))?;
    super::terminal_authority_permissions::validate_retained_package_terminal_authority_permissions(
        proposal.package_terminal_authority_permissions(),
        &accepted_package_terminal_authority_permission_policy,
    )?;
    super::terminal_authority_permissions::validate_package_terminal_authority_permissions(
        accepted_package_terminal_authority_permission_policy
            .rows()
            .iter(),
        &terminal_authority_permission_policy,
    )?;
    let native_callbacks =
        admitted_native_callbacks(&callback_placements, proposal.callback_occurrences())?;
    let callback_thunks =
        admitted_native_callback_thunks(&callback_placements, proposal.callback_occurrences())?;
    debug_assert_eq!(callback_placements.len(), native_callbacks.len());
    debug_assert_eq!(callback_placements.len(), callback_thunks.len());

    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        diagnostic(
            "Terminal native proposal",
            format!("canonical semantics could not be decoded: {error}"),
        )
    })?;
    let demanded = super::intrinsic_settlements::demanded_boundary_identities(&module)?;
    let exact_import_plans = exact_demanded_import_plans(&proposal, &demanded)?;
    let native_settlements = rejoin_external_import_settlements(&exact_import_plans, imports)?;

    let selected_plans = proposal.selected_provider_plans().plans();
    let compiler_builtins = proposal
        .compiler_builtins()
        .iter()
        .map(|builtin| {
            let provider_plan = selected_plans
                .get(builtin.provider_plan_index())
                .ok_or_else(|| {
                    diagnostic(
                        "Terminal native proposal",
                        format!(
                            "compiler builtin `{}` names an absent selected provider plan",
                            builtin.requirement_identity()
                        ),
                    )
                })?;
            Ok(
                omega_terminal_psi_to_native_artifact::NativeCompilerBuiltinSettlement {
                    requirement_identity: builtin.requirement_identity(),
                    provider_plan,
                    execution: builtin.execution(),
                },
            )
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let ieee_float_fma = proposal
        .ieee_float_fma_occurrences()
        .iter()
        .map(|occurrence| {
            let provider_plan = proposal
                .selected_provider_plans()
                .plans()
                .get(occurrence.provider_plan_index())
                .ok_or_else(|| {
                    diagnostic(
                        "Terminal nearest-FMA proposal",
                        "occurrence names an absent exact selected provider plan",
                    )
                })?;
            let admission = occurrence.x86_admission().ok_or_else(|| {
                diagnostic(
                    "Terminal nearest-FMA proposal",
                    "ordinary native lowering currently requires admitted x86 FMA custody",
                )
            })?;
            Ok(
                omega_terminal_psi_to_native_artifact::AdmittedIeeeFloatFmaSettlement {
                    terminal_operation: occurrence.terminal_operation(),
                    provider_plan,
                    format: occurrence.format(),
                    slot: admission.slot(),
                    provider: admission.provider(),
                },
            )
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let calling_plans = proposal
        .program_entry()
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        proposal.program_entry().source_signature(),
        calling_plans,
    );
    omega_terminal_psi_to_native_artifact::realize_native_artifact_with_checked_boundary_operator_scope(
        artifact,
        proposal.checked_boundary_operator_scope(),
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target: proposal.native_target(),
            subsystem: proposal.subsystem(),
            profile,
            terminal_authority_policy,
            terminal_authority_permission_policy,
            program_entry,
            optimization_selections,
            selected_provider_plans: proposal.selected_provider_plans(),
            external_binding_rows: proposal.external_binding_rows(),
            settlements: &native_settlements,
            compiler_builtins: &compiler_builtins,
            boundary_application_coverage: Some(proposal.boundary_application_coverage()),
            ieee_float_fma: &ieee_float_fma,
            native_callbacks: &native_callbacks,
            callback_thunks: &callback_thunks,
        },
    )
}

fn admitted_native_callback_thunks<'artifact>(
    placements: &'artifact [omega_backend_plan::BoundNominalCallbackPlacement],
    occurrences: &'artifact [omega_compilation_report::TerminalCallbackOccurrenceProposal],
) -> Result<
    Vec<omega_terminal_psi_to_native_artifact::NativeCallbackThunkSettlement<'artifact>>,
    Vec<Diagnostic>,
> {
    let mut admitted = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let placement = placements
            .get(occurrence.placement_index())
            .ok_or_else(|| {
                diagnostic(
                    "Terminal callback thunk custody",
                    "callback thunk occurrence names an absent retained placement",
                )
            })?;
        let thunk = occurrence.callback_thunk_artifact();
        let receipt = thunk.lowering_receipt();
        let expected_symbol = omega_backend_plan::canonical_callback_private_symbol(placement);
        let expected_function = omega_backend_plan::canonical_callback_thunk_identity(
            occurrence.placement_index(),
            placement,
        );
        if receipt.source_machine != placement.selected_machine
            || receipt.source_entry != placement.selected_entry
            || thunk.private_symbol() != &expected_symbol
            || expected_function != Some(occurrence.callback_thunk_identity())
        {
            return Err(diagnostic(
                "Terminal callback thunk custody",
                "callback thunk body, symbol, or function identity drifted from its retained placement",
            ));
        }
        admitted.push(
            omega_terminal_psi_to_native_artifact::NativeCallbackThunkSettlement {
                terminal_operation: occurrence.terminal_operation(),
                placement_index: occurrence.placement_index(),
                callback_function: occurrence.callback_thunk_identity(),
                private_symbol: thunk.private_symbol(),
                artifact: thunk.artifact(),
                lowering_receipt: receipt,
                boundary_entry_plan: &placement.boundary_entry_plan,
            },
        );
    }
    Ok(admitted)
}

fn admitted_native_callbacks(
    placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    occurrences: &[omega_compilation_report::TerminalCallbackOccurrenceProposal],
) -> Result<
    Vec<omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument>,
    Vec<Diagnostic>,
> {
    if placements.len() > 1 || occurrences.len() > 1 {
        return Err(diagnostic(
            "Terminal callback custody",
            "ordinary native realization currently admits exactly one direct callback",
        ));
    }
    let mut admitted = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let placement = placements
            .get(occurrence.placement_index())
            .ok_or_else(|| {
                diagnostic(
                    "Terminal callback custody",
                    "callback occurrence names an absent retained placement",
                )
            })?;
        let materialization = placement.private_materialization.as_ref().ok_or_else(|| {
            diagnostic(
                "Terminal callback custody",
                "callback placement has no private registrar materialization",
            )
        })?;
        let application = occurrence.direct_parameter_application().ok_or_else(|| {
            diagnostic(
                "Terminal callback custody",
                "field callback materialization is outside the direct-parameter cohort",
            )
        })?;
        if materialization
            .direct_registrar_parameter_application
            .as_ref()
            != Some(application)
            || materialization.destination
                != omega_calling_conventions::NativePlace::Parameter(application.parameter)
        {
            return Err(diagnostic(
                "Terminal callback custody",
                "direct callback application drifted from its retained registrar materialization",
            ));
        }
        admitted.push(
            omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument {
                terminal_operation: occurrence.terminal_operation(),
                placement_index: occurrence.placement_index(),
                callback_function: occurrence.callback_thunk_identity(),
                application: application.clone(),
                registrar_boundary_entry_plan: materialization
                    .registrar_boundary_entry_plan
                    .clone(),
                registrar_context: materialization.context.clone(),
                registrar_application_commitment: materialization.registrar_application_commitment,
            },
        );
    }
    Ok(admitted)
}

fn exact_demanded_import_plans<'proposal>(
    proposal: &'proposal omega_compilation_report::TerminalNativeRealizationProposal,
    demanded: &BTreeSet<String>,
) -> Result<BTreeMap<String, &'proposal omega_effects::provider_plan::ProviderPlan>, Vec<Diagnostic>>
{
    let mut exact = BTreeMap::new();
    for requirement in demanded {
        let matches = proposal
            .selected_provider_plans()
            .plans()
            .iter()
            .flat_map(|plan| {
                plan.rows
                    .iter()
                    .filter(move |row| {
                        row.requirement_identity == *requirement
                            && matches!(
                                row.binding,
                                omega_effects::provider_plan::ProviderBinding::Import { .. }
                            )
                    })
                    .map(move |_| plan)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [plan] => {
                exact.insert(requirement.clone(), *plan);
            }
            _ => {
                return Err(diagnostic(
                    "Terminal native proposal",
                    format!(
                        "demanded source-evaluated import `{requirement}` resolves to {} selected import plans",
                        matches.len()
                    ),
                ));
            }
        }
    }
    Ok(exact)
}

fn rejoin_external_import_settlements<'proposal, 'evidence>(
    exact_plans: &BTreeMap<String, &'proposal omega_effects::provider_plan::ProviderPlan>,
    imports: &[SourceEvaluatedImportSettlement<'evidence>],
) -> Result<
    Vec<omega_terminal_psi_to_native_artifact::NativeProviderSettlement<'proposal>>,
    Vec<Diagnostic>,
>
where
    'evidence: 'proposal,
{
    let mut seen = BTreeSet::new();
    let mut settlements = Vec::with_capacity(imports.len());
    for import in imports {
        let requirement = import.provider_execution.requirement_identity();
        if !seen.insert(requirement) {
            return Err(diagnostic(
                "source-evaluated import settlement",
                format!("requirement `{requirement}` was supplied more than once"),
            ));
        }
        let provider_plan = exact_plans.get(requirement).copied().ok_or_else(|| {
            diagnostic(
                "source-evaluated import settlement",
                format!("requirement `{requirement}` is not a demanded selected import"),
            )
        })?;
        if provider_plan.report_fingerprint()
            != import.provider_execution.provider_plan_report_identity()
        {
            return Err(diagnostic(
                "source-evaluated import settlement",
                format!(
                    "execution evidence for `{requirement}` names a different provider-plan report coordinate"
                ),
            ));
        }
        settlements.push(
            omega_terminal_psi_to_native_artifact::NativeProviderSettlement {
                provider_execution: import.provider_execution,
                provider_plan,
                realization:
                    omega_terminal_psi_to_native_artifact::NativeBoundaryRealization::NormalizedForeignCall(
                        import.same_stack,
                    ),
            },
        );
    }
    if let Some(missing) = exact_plans
        .keys()
        .find(|requirement| !seen.contains(requirement.as_str()))
    {
        return Err(diagnostic(
            "source-evaluated import settlement",
            format!("demanded import `{missing}` has no supplied execution and stack custody"),
        ));
    }
    Ok(settlements)
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
