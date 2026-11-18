//! Plan every reachable private native body before physical resource planning begins.
use std::sync::Arc;
use backend_plan::NativeCallbackThunkSettlement;
use diagnostics::Diagnostic;
use provider_admission::provider_admission::{AdmittedInstalledInterruptCohort, realization_error};
use crate::{NativeBodyPlanningError, admit_prepared_native_realization};

pub struct NativeCallbackPlanningRequest<'request, 'contract> {
    pub proposal: &'contract crate::TerminalNativeRealizationProposal,
    pub execution: &'contract crate::PreparedNativeExecution,
    pub scalar_applications: Option<&'contract terminal_installation::AdmittedScalarApplications>,
    pub admission: &'request crate::NativeAdmissionRequest<'contract, 'contract>,
    pub callback_thunks: &'contract [NativeCallbackThunkSettlement<'contract>],
    pub optimization_selections: &'contract optimization_core::PostTerminalOptimizationSelections,
    pub budget: optimization_core::OptimizationWorkBudget,
    pub retain_native_stage_inputs: bool,
}

pub struct CallableTargetProduct {
    pub retain_native_stage_inputs: bool,
    pub code: terminal_codec::RetainedTerminalCode,
    pub coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    pub target: crate::NativeBodyPlan,
    pub runtime: native_runtime_provisions::TerminalCallableRuntimeProvisions<
        native_runtime_provisions::NativeForeignAdapterRequest,
    >,
    pub authority: provider_admission::provider_admission::AdmittedNativeCallableAuthority,
    pub observation: crate::observations::NativePipelineObservation,
}

pub fn plan_callable(
    mut admitted: crate::AdmittedNativeRealization<'_>,
    admission: crate::NativeAdmissionRequest<'_, '_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    retain_native_stage_inputs: bool,
) -> Result<CallableTargetProduct, Vec<Diagnostic>> {
    let observation = crate::observations::NativePipelineObservation::new(
        admission.providers.profile,
        crate::observations::NativePipelineUnit::Callable,
    );
    let callable_entry = admitted.input.plan().entry;
    let authority = admitted.providers.callable_authority().clone();
    let host = native_runtime_provisions::plan_host_provisions(
        admitted
            .providers
            .installation
            .as_ref()
            .map(|installation| installation.terminal_selection()),
        admitted
            .providers
            .settlements
            .iter()
            .map(|settlement| native_runtime_provisions::TerminalRuntimeHostProvision {
                boundary: settlement.boundary,
                realization: settlement.realization.clone(),
            })
            .collect(),
        admitted.input.optimization_input().runtime_input(),
        admission.providers.selected_provider_plans,
        admission.providers.external_binding_rows,
        native_runtime_provisions::NativeBindingRuntimeInputs {
            bindings: admission.providers.native_bindings,
            calling: admission.providers.installed_calling,
        },
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let runtime = native_runtime_provisions::TerminalCallableRuntimeProvisions::admit(
        artifact.view(),
        admitted.input.optimization_input().runtime_input(),
        callable_entry,
        admission.providers.profile,
        host,
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let optimized_target = crate::plan_native_body(
        admitted.input,
        crate::OptimizedTargetLoweringRequest {
            target: admission.providers.target(),
            settlements: &admitted.providers.settlements,
            installation: admitted.providers.installation.take(),
        },
        selections,
        budget,
        observation.native_body(),
        retain_native_stage_inputs,
    )
    .map_err(|error| match error {
        NativeBodyPlanningError::AbstractOptimization(error) => {
            realization_error("callable abstract optimization", error)
        }
        NativeBodyPlanningError::Target(error) => {
            realization_error("callable target lowering", error)
        }
    })?;
    let runtime = runtime.with_borrowed_storage_layouts(
        optimized_target
            .execution_plan()
            .plan()
            .borrowed_storage_layouts
            .clone(),
    );
    Ok(CallableTargetProduct {
        retain_native_stage_inputs,
        code: artifact.retain_code(),
        coverage: admission
            .boundary_application_coverage
            .ok_or_else(|| {
                realization_error(
                    "callable physical evidence",
                    "callable has no semantic occurrence coverage",
                )
            })?
            .clone(),
        target: optimized_target,
        runtime,
        authority,
        observation,
    })
}

/// Physical planning receives only the selected private targets, their ABI
/// contracts and artifact identities. Source lookup and native lowering end
/// before any body asks for a numeric installed-handler stack guard.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub struct PrivateEntryContract<'a> {
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub psi: terminal_psi::TerminalPsiIdentity,
    pub entry: semantic_vocabulary::MachineId,
    pub boundary_entry_plan: &'a calling_conventions::BoundaryEntryPlan,
    pub parameter_formals: &'a [u32],
    pub entry_parameters: &'a [backend_plan::NativeCallbackEntryParameter],
}
impl<'a> From<&NativeCallbackThunkSettlement<'a>> for PrivateEntryContract<'a> {
    fn from(row: &NativeCallbackThunkSettlement<'a>) -> Self {
        Self {
            terminal_operation: row.terminal_operation,
            placement_index: row.placement_index,
            callback_function: row.callback_function,
            psi: row.artifact.view().identity(),
            entry: row.entry,
            boundary_entry_plan: row.boundary_entry_plan,
            parameter_formals: row.parameter_formals,
            entry_parameters: row.entry_parameters,
        }
    }
}

pub struct ClosedCallbackRoster<'contract> {
    placement_count: usize,
    bodies: Vec<(PrivateEntryContract<'contract>, CallableTargetProduct)>,
}
impl<'contract> ClosedCallbackRoster<'contract> {
    pub fn into_parts(self) -> (usize, Vec<(PrivateEntryContract<'contract>, CallableTargetProduct)>) {
        (self.placement_count, self.bodies)
    }

    pub fn plan(
        request: &NativeCallbackPlanningRequest<'_, 'contract>,
        program_provider_context: &Arc<
            provider_admission::provider_admission::AdmittedProgramProviderContext,
        >,
        root: &crate::NativeBodyPlan,
        native_callbacks: &[provider_admission::AdmittedNativeCallbackArgument],
        installed_interrupts: &AdmittedInstalledInterruptCohort,
    ) -> Result<Self, Vec<Diagnostic>> {
        let placement_count = request.callback_thunks.len();
        let mut placements = vec![false; placement_count];
        for row in request.callback_thunks {
            let Some(slot) = placements.get_mut(row.placement_index) else {
                return Err(realization_error(
                    "callback body closure",
                    "placement falls outside complete original roster",
                ));
            };
            if std::mem::replace(slot, true) {
                return Err(realization_error(
                    "callback body closure",
                    "original callback placement is duplicated",
                ));
            }
        }
        if installed_interrupts.psi() != root.execution_plan().plan().psi {
            return Err(realization_error(
                "installed interrupt callback emission",
                "admitted entries and physical program belong to different Terminal programs",
            ));
        }
        let mut pending = request
            .callback_thunks
            .iter()
            .filter(|settlement| {
                root.private_references
                    .contains(&settlement.callback_function)
                    && (native_callbacks.iter().any(|callback| {
                        callback.terminal_operation == settlement.terminal_operation
                            && callback.placement_index == settlement.placement_index
                            && callback.callback_function == settlement.callback_function
                    }) || installed_interrupts.entries().iter().any(|entry| {
                        entry.operation == settlement.terminal_operation
                            && entry.placement_index == settlement.placement_index
                            && entry.callback_function == settlement.callback_function
                    }))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut bodies = Vec::with_capacity(pending.len());
        let mut next_callback = 0;
        while next_callback < pending.len() {
            let settlement = pending[next_callback];
            let target = plan_callback_target(request, &settlement, program_provider_context)?;
            for child in request.callback_thunks.iter().copied() {
                if !target
                    .target
                    .private_references
                    .contains(&child.callback_function)
                {
                    continue;
                }
                if let Some(known) = pending
                    .iter()
                    .find(|known| known.callback_function == child.callback_function)
                {
                    if known.artifact.manifest().identity() != child.artifact.manifest().identity()
                        || known.entry != child.entry
                        || known.boundary_entry_plan != child.boundary_entry_plan
                        || known.parameter_formals != child.parameter_formals
                        || known.entry_parameters != child.entry_parameters
                    {
                        return Err(realization_error(
                            "callback body closure",
                            "one private identity selects incompatible bodies or entry plans",
                        ));
                    }
                } else {
                    pending.push(child);
                }
            }
            bodies.push((PrivateEntryContract::from(&settlement), target));
            next_callback += 1;
        }
        Ok(Self { placement_count, bodies })
    }
}

fn plan_callback_target(
    request: &NativeCallbackPlanningRequest<'_, '_>,
    settlement: &NativeCallbackThunkSettlement<'_>,
    program_provider_context: &Arc<
        provider_admission::provider_admission::AdmittedProgramProviderContext,
    >,
) -> Result<CallableTargetProduct, Vec<Diagnostic>> {
    let admission = crate::NativeAdmissionRequest {
        private_entry_ram: &[],
        entry: terminal_psi::TargetMachineCallable::Machine(settlement.entry),
        providers: provider_admission::provider_admission::ProviderAdmissionRequest {
            program_provider_context: Some(program_provider_context),
            ..request.admission.providers
        },
        ..*request.admission
    };
    let (input, boundary_coverage) = crate::lower_native_callable(
        request.execution,
        settlement.artifact,
        request.proposal,
        admission.entry,
        request.scalar_applications.map(|completed| crate::NativeScalarApplications { completed }),
    )?;
    let admission = crate::NativeAdmissionRequest {
        boundary_application_coverage: Some(&boundary_coverage),
        ..admission
    };
    let admitted = admit_prepared_native_realization(input, settlement.artifact, &admission)?;
    plan_callable(
        admitted,
        admission,
        request.optimization_selections,
        request.budget,
        settlement.artifact,
        request.retain_native_stage_inputs,
    )
}
