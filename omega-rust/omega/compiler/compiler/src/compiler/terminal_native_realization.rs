//! Native re-entry for a retained Terminal product with source-evaluated imports.
//!
//! This coordinator owns the exact-plan join. Callers supply only independently
//! admitted execution and same-stack evidence; they cannot select a provider
//! plan, locator, target, compiler builtin, or checked-scope receipt.

use diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;
use std::collections::{BTreeMap, BTreeSet};
use task_plans::AdmittedSameStackContribution;

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

/// Explicit policy, image custody, and admitted imports for native re-entry.
///
/// Policies do not establish package admission. Package orchestration must bind
/// opaque accepted evidence to the retained production subject and derive the
/// exact accepted-package policy before constructing this request.
pub struct RetainedNativeRealizationRequest<'request> {
    pub profile: &'request proof_admission::AdmissionProfile,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: native_realization::TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy:
        native_realization::TerminalAuthorityPermissionPolicy,
    pub terminal_authority_permission_policy: native_realization::TerminalAuthorityPermissionPolicy,
    pub image_request: native_realization::ExecutableImageEmissionRequest,
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
}

/// Consume a retained Terminal product into the requested non-installing native
/// carrier. Rejection returns the complete image input, including its interpreter.
///
/// Package permissions must exactly match the retained proposal. Receiving
/// permissions may add unrelated rows, but cannot omit or alter accepted rows.
/// Imports are independently rejoined to their exact retained provider plans.
pub fn realize_retained_native_artifact(
    retained: compilation_report::RetainedTerminalArtifact,
    request: RetainedNativeRealizationRequest<'_>,
) -> Result<
    native_realization::RequestedNativeArtifact,
    (
        native_realization::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let RetainedNativeRealizationRequest {
        profile,
        optimization_selections,
        terminal_authority_policy,
        accepted_package_terminal_authority_permission_policy,
        terminal_authority_permission_policy,
        image_request,
        imports,
    } = request;
    let recoverable_image_request = image_request.clone();
    let result = (|| {
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
        // Proposal validation rejoins each IEEE comparison to its exact selected
        // intrinsic and checked arm/expression occurrence. The ordinary graph
        // lowering preserves that operation identity into the D29 physical child;
        // bitwise IEEE ordering needs no independent foreign-provider settlement.
        if proposal.post_terminal_optimizations().selections() != optimization_selections {
            return Err(diagnostic(
                "Terminal native proposal",
                "receiving lowerer selections differ from the exact post-Terminal build proposal",
            ));
        }
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

        let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
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
                Ok(native_realization::NativeCompilerBuiltinSettlement {
                    requirement_identity: builtin.requirement_identity(),
                    provider_plan,
                    execution: builtin.execution(),
                })
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
                Ok(native_realization::AdmittedIeeeFloatFmaSettlement {
                    terminal_operation: occurrence.terminal_operation(),
                    provider_plan,
                    format: occurrence.format(),
                    slot: admission.slot(),
                    provider: admission.provider(),
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let calling_plans = proposal.program_entry().calling_plans().map(|plans| {
            (
                &plans.semantic_calling_application,
                &plans.physical_calling_application,
                &plans.storage_entry,
            )
        });
        let program_entry = native_realization::NativeProgramEntrySettlement::new(
            proposal.program_entry().source_signature(),
            calling_plans,
            proposal.program_entry().fused_service_establishments(),
        );
        native_realization::realize_requested_native_artifact_with_checked_boundary_operator_scope(
            artifact,
            proposal.checked_boundary_operator_scope(),
            native_realization::RequestedNativeRealizationRequest {
                target: proposal.native_target(),
                image_request,
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
        .map_err(|error| error.into_parts().1)
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}

fn admitted_native_callback_thunks<'artifact>(
    placements: &'artifact [backend_plan::BoundNominalCallbackPlacement],
    occurrences: &'artifact [compilation_report::TerminalCallbackOccurrenceProposal],
) -> Result<Vec<native_realization::NativeCallbackThunkSettlement<'artifact>>, Vec<Diagnostic>> {
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
        let expected_symbol = backend_plan::canonical_callback_private_symbol(placement);
        let expected_function = backend_plan::canonical_callback_thunk_identity(
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
        admitted.push(native_realization::NativeCallbackThunkSettlement {
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            private_symbol: thunk.private_symbol(),
            artifact: thunk.artifact(),
            lowering_receipt: receipt,
            boundary_entry_plan: &placement.boundary_entry_plan,
        });
    }
    Ok(admitted)
}

fn admitted_native_callbacks(
    placements: &[backend_plan::BoundNominalCallbackPlacement],
    occurrences: &[compilation_report::TerminalCallbackOccurrenceProposal],
) -> Result<
    Vec<abstract_operations_to_target_operations::AdmittedNativeCallbackArgument>,
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
                != calling_conventions::NativePlace::Parameter(application.parameter)
        {
            return Err(diagnostic(
                "Terminal callback custody",
                "direct callback application drifted from its retained registrar materialization",
            ));
        }
        admitted.push(
            abstract_operations_to_target_operations::AdmittedNativeCallbackArgument {
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
    proposal: &'proposal compilation_report::TerminalNativeRealizationProposal,
    demanded: &BTreeSet<String>,
) -> Result<BTreeMap<String, &'proposal effects::provider_plan::ProviderPlan>, Vec<Diagnostic>> {
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
                                effects::provider_plan::ProviderBinding::Import { .. }
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
    exact_plans: &BTreeMap<String, &'proposal effects::provider_plan::ProviderPlan>,
    imports: &[SourceEvaluatedImportSettlement<'evidence>],
) -> Result<Vec<native_realization::NativeProviderSettlement<'proposal>>, Vec<Diagnostic>>
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
        settlements.push(native_realization::NativeProviderSettlement {
            provider_execution: import.provider_execution,
            provider_plan,
            realization: native_realization::NativeBoundaryRealization::NormalizedForeignCall(
                import.same_stack,
            ),
        });
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
