//! Check provider admission against the selected abstract program and its authorized Psi.
use crate::{ArtifactLoweringError, VerifiedNativeArtifactInput};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use provider_admission::{
    AdmittedNativeProviders, NativeProviderSettlement, PreparedSourceEvaluatedBindings,
    ProviderAdmissionRequest, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
    admit_native_providers, prepare_source_evaluated_bindings, realization_error,
};
use provider_realization::native_execution::PreparedNativeExecution;
use semantic_vocabulary::BoundaryMachineId;
use std::collections::BTreeSet;
use terminal_codec::CanonicalTerminalArtifact;
use verified_optimization_input::TerminalNativeRealizationProposal;

pub struct RetainedNativeProviderInputs<'input> {
    pub execution: &'input PreparedNativeExecution,
    pub optimization_selections:
        &'input optimization_selections::PostTerminalOptimizationSelections,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
}

pub struct PreparedRetainedNativeProviders<'input> {
    execution: &'input PreparedNativeExecution,
    proposal: &'input TerminalNativeRealizationProposal,
    bindings: PreparedSourceEvaluatedBindings<'input>,
    callbacks: &'input [crate::NativeCallbackArgumentProposal],
}

impl PreparedRetainedNativeProviders<'_> {
    pub fn program_settlements(&self) -> Vec<NativeProviderSettlement<'_>> {
        self.bindings
            .settlements()
            .iter()
            .filter(|settlement| {
                self.execution
                    .demands()
                    .contains(settlement.provider_execution.requirement_identity())
            })
            .copied()
            .collect()
    }

    pub fn admission_request<'request>(
        &'request self,
        settlements: &'request [NativeProviderSettlement<'request>],
        terminal_authority_policy: &'request TerminalAuthorityPolicy,
        receiving_permissions: &'request Option<TerminalAuthorityPermissionPolicy>,
        isolated_execution_grant: Option<
            &'request semantic_vocabulary::TerminalAuthorityDisposition,
        >,
        installation_assumptions: &'request [installation_conditions::InstallationEnvironmentAssumption],
    ) -> crate::NativeAdmissionRequest<'request, 'request> {
        let proposal = self.proposal;
        crate::NativeAdmissionRequest {
            private_entry_ram: &[],
            entry: terminal_psi::TargetMachineCallable::Machine(
                proposal.scope_source().root().semantic_entry(),
            ),
            boundary_application_coverage: Some(self.execution.boundary_application_coverage()),
            behavior_exclusions: proposal.behavior_exclusions(),
            isolated_execution_grant,
            providers: ProviderAdmissionRequest {
                profile: proposal.target_profile(),
                bootstrap_entry: proposal.scope_source().root().semantic_entry(),
                program_provider_context: None,
                installation_assumptions,
                selected_provider_plans: self.execution.selection(),
                checked_adapters: self.execution.local_adapters(),
                external_binding_rows: self.execution.external_bindings(),
                settlements,
                compiler_builtins: self.execution.compiler_builtins(),
                native_bindings: &proposal.providers().native_bindings,
                native_callbacks: self.callbacks,
                installed_interrupts: &proposal.occurrences().installed_interrupts,
                installed_calling: &proposal.boundary_applications().calling,
                callback_occurrences: &proposal.occurrences().callbacks,
                terminal_authority_policy,
                terminal_authority_permission_policy: receiving_permissions,
                x86_scalar_fma_provider: proposal.providers().x86_scalar_fma,
            },
        }
    }
}

/// Lower a native compilation root and prepare the providers from that same selected execution.
pub fn lower_native_root<'input>(
    proposal: &'input TerminalNativeRealizationProposal,
    inputs: RetainedNativeProviderInputs<'input>,
    private_entry_ram: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
    scalars: Option<crate::NativeScalarApplications<'_>>,
) -> Result<
    (
        VerifiedNativeArtifactInput,
        PreparedRetainedNativeProviders<'input>,
    ),
    Vec<Diagnostic>,
> {
    let root = crate::selected_execution::lower_native_execution(
        proposal,
        inputs.execution,
        private_entry_ram,
        scalars,
    )?;
    let providers = prepare_native_providers(proposal, inputs)?;
    Ok((root, providers))
}

pub fn prepare_native_providers<'input>(
    proposal: &'input TerminalNativeRealizationProposal,
    inputs: RetainedNativeProviderInputs<'input>,
) -> Result<PreparedRetainedNativeProviders<'input>, Vec<Diagnostic>> {
    if proposal.post_terminal_optimizations().selections() != inputs.optimization_selections {
        return Err(diagnostics::error_in_context(
            "Terminal native proposal",
            "receiving lowerer selections differ from the exact post-Terminal build proposal",
        ));
    }
    provider_admission::terminal_authority_permissions::validate_retained_package_terminal_authority_permissions(
        proposal.package_terminal_authority_permissions(),
        proposal.package_terminal_authority_permission_identities(),
        inputs.accepted_package_permissions,
    )?;
    let callbacks = proposal
        .occurrences()
        .callback_arguments
        .as_deref()
        .ok_or_else(|| {
            diagnostics::error_in_context(
                "Terminal callback custody",
                "callback placement has no private registrar materialization",
            )
        })?;
    let bindings = prepare_source_evaluated_bindings(
        inputs.execution.selection().plans(),
        inputs.execution.demands(),
        &[],
        inputs.execution.selection().identities(),
    )?;

    Ok(PreparedRetainedNativeProviders {
        execution: inputs.execution,
        proposal,
        bindings,
        callbacks,
    })
}

#[derive(Clone, Copy)]
pub struct NativeAdmissionRequest<'request, 'execution> {
    pub entry: terminal_psi::TargetMachineCallable,
    pub private_entry_ram: &'request [psi_entry_memory::CheckedPrivateEntryRamRecipe],
    pub providers: ProviderAdmissionRequest<'request, 'execution>,
    pub boundary_application_coverage: Option<&'request TerminalBoundaryApplicationCoverage>,
    pub behavior_exclusions: &'request native_behavior_exclusions::BehaviorExclusions,
    pub isolated_execution_grant:
        Option<&'request semantic_vocabulary::TerminalAuthorityDisposition>,
}

pub struct AdmittedNativeRealization<'execution> {
    pub input: VerifiedNativeArtifactInput,
    pub providers: AdmittedNativeProviders<'execution>,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
}

pub fn admit_prepared_native_realization<'execution>(
    input: VerifiedNativeArtifactInput,
    artifact: &CanonicalTerminalArtifact,
    request: &NativeAdmissionRequest<'_, 'execution>,
) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
    input
        .context()
        .authorization()
        .require_artifact(artifact)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let executed_boundaries: BTreeSet<BoundaryMachineId> = input
        .plan()
        .boundary_machines
        .iter()
        .map(|boundary| boundary.id)
        .collect();
    let settlements = request
        .providers
        .settlements
        .iter()
        .copied()
        .filter(|settlement| {
            let mut matching = input
                .context()
                .module()
                .interfaces.boundary_machines
                .iter()
                .filter(|boundary| {
                    request
                        .providers
                        .selected_provider_plans
                        .selected_row_matches_requirement(
                            input
                                .context()
                                .module()
                                .identities
                                .text(boundary.identity)
                                .unwrap_or_default(),
                            settlement.provider_execution.requirement_identity(),
                            settlement
                                .provider_execution
                                .provider_plan_report_identity(),
                        )
                })
                .peekable();
            matching.peek().is_none()
                || matching.any(|boundary| executed_boundaries.contains(&boundary.id))
        })
        .collect::<Vec<_>>();
    let (native_callbacks, installed_interrupts) = if request
        .providers
        .native_callbacks
        .is_empty()
        && request.providers.installed_interrupts.is_empty()
    {
        (Vec::new(), Vec::new())
    } else {
        let plan_boundary_call_ops: BTreeSet<_> = input
            .plan()
            .functions
            .iter()
            .flat_map(|function| {
                function
                    .operations()
                    .filter_map(move |operation| match operation {
                        abstract_operations::AbstractOperation::BoundaryCall {
                            psi_operation,
                            ..
                        } => Some((function.machine, *psi_operation)),
                        _ => None,
                    })
            })
            .collect();
        let module_boundary_call_ops: BTreeSet<_> = input
            .context()
            .module()
            .machines
            .iter()
            .flat_map(|machine| {
                machine.operations().filter_map(move |operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::BoundaryCall { .. }
                    )
                    .then_some((machine.id, operation.id))
                })
            })
            .collect();
        let in_scope = |machine, operation| {
            plan_boundary_call_ops.contains(&(machine, operation))
                || !module_boundary_call_ops.contains(&(machine, operation))
        };
        let native_callbacks = request
            .providers
            .native_callbacks
            .iter()
            .filter(|callback| {
                in_scope(callback.registrar_machine, callback.terminal_operation)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut callback_occurrences = vec![None; request.providers.callback_occurrences.len()];
        for occurrence in request.providers.callback_occurrences {
            let Some(slot) = callback_occurrences.get_mut(occurrence.placement_index()) else {
                return Err(realization_error(
                    "installed interrupt callback scope",
                    "callback placement falls outside the original complete roster",
                ));
            };
            if slot.replace(occurrence).is_some() {
                return Err(realization_error(
                    "installed interrupt callback scope",
                    "original callback placement identity is duplicated",
                ));
            }
        }
        let installed_interrupts = request
            .providers
            .installed_interrupts
            .iter()
            .filter(|entry| {
                in_scope(entry.machine, entry.operation)
                    || callback_occurrences
                        .get(entry.placement_index)
                        .copied()
                        .flatten()
                        .is_some_and(|occurrence| {
                            occurrence.callback_thunk_artifact().entry() == input.plan().entry
                                && occurrence.terminal_operation() == entry.operation
                                && occurrence.application_index()
                                    == entry.placement.application_index
                                && occurrence.callback_thunk_identity()
                                    == entry.callback_function
                        })
            })
            .cloned()
            .collect::<Vec<_>>();
        (native_callbacks, installed_interrupts)
    };
    let scoped_providers = ProviderAdmissionRequest {
        profile: request.providers.profile,
        bootstrap_entry: request.providers.bootstrap_entry,
        program_provider_context: request.providers.program_provider_context,
        installation_assumptions: request.providers.installation_assumptions,
        selected_provider_plans: request.providers.selected_provider_plans,
        checked_adapters: request.providers.checked_adapters,
        external_binding_rows: request.providers.external_binding_rows,
        settlements: &settlements,
        compiler_builtins: request.providers.compiler_builtins,
        native_bindings: request.providers.native_bindings,
        native_callbacks: &native_callbacks,
        installed_interrupts: &installed_interrupts,
        installed_calling: request.providers.installed_calling,
        callback_occurrences: request.providers.callback_occurrences,
        terminal_authority_policy: request.providers.terminal_authority_policy,
        terminal_authority_permission_policy: request
            .providers
            .terminal_authority_permission_policy,
        x86_scalar_fma_provider: request.providers.x86_scalar_fma_provider,
    };
    let mut providers =
        admit_native_providers(&input.admission_source(), artifact, &scoped_providers)?;
    input.optimization_input().validate_behavior_exclusions(
        request.behavior_exclusions,
        scoped_providers.selected_provider_plans,
        scoped_providers.profile,
    )?;
    request
        .behavior_exclusions
        .validate_physical_authority_closure(providers.terminal_authority_closure_review())?;
    if let Some(grant) = request.isolated_execution_grant {
        providers.check_isolated_execution_grant(grant)?;
    }
    let mut input = input;
    input.retain_admitted_assumed_bindings(
        providers
            .installation
            .as_ref()
            .map(|installation| {
                installation
                    .terminal_selection()
                    .assumed_bindings()
                    .iter()
                    .map(|row| {
                        operation_evidence::provider_calls::InstalledAssumedBindingEvidence {
                            required: row.required,
                            supplied_requirement: row.supplied_requirement,
                            implementation: row.implementation,
                            selected_requirement: row.selected_requirement,
                            supply_path: row.supply_path.clone(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
    );
    input.retain_admitted_native_callbacks(providers.take_native_callbacks());
    let installed_interrupts = providers.take_installed_interrupts();
    if installed_interrupts.psi() != input.plan().psi {
        return Err(vec![Diagnostic::error(
            "installed interrupt entries belong to another Terminal program",
        )]);
    }
    input.retain_admitted_installed_interrupts(installed_interrupts);
    Ok(AdmittedNativeRealization {
        input,
        providers,
        boundary_application_coverage: request.boundary_application_coverage.cloned(),
    })
}

pub fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
    realization_error("native artifact lowering", error)
}
