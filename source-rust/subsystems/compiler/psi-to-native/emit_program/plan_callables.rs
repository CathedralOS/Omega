//! Plan every reachable private native body before physical resource planning begins.
use provider_realization::native_execution::PreparedNativeExecution;
use verified_optimization_input::TerminalNativeRealizationProposal;
use crate::compile_body::NativeBodyPlanningError;
use artifact_lowering::admit_prepared_native_realization;
use diagnostics::Diagnostic;
use provider_admission::realization_error;
use provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal;
use std::sync::Arc;

pub struct NativeProgramPlanningRequest<'request, 'contract> {
    pub proposal: &'contract TerminalNativeRealizationProposal,
    pub execution: &'contract PreparedNativeExecution,
    pub scalar_applications: Option<&'contract psi_scalar_applications::AdmittedScalarApplications>,
    pub admission: &'request artifact_lowering::NativeAdmissionRequest<'contract, 'contract>,
    pub optimization_selections: &'contract optimization_core::PostTerminalOptimizationSelections,
    pub budget: optimization_core::OptimizationWorkBudget,
    pub retain_native_stage_inputs: bool,
}

pub struct CallableTargetProduct {
    pub code: terminal_codec::RetainedTerminalCode,
    pub coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    pub target: target_lowering::NativeBodyPlan,
    pub runtime: crate::runtime_provisions::TerminalCallableRuntimeProvisions<
        crate::runtime_provisions::NativeForeignAdapterRequest,
    >,
    pub authority: provider_admission::AdmittedNativeCallableAuthority,
    pub observation: crate::observations::NativePipelineObservation,
}

pub fn plan_callable(
    mut admitted: artifact_lowering::AdmittedNativeRealization<'_>,
    admission: artifact_lowering::NativeAdmissionRequest<'_, '_>,
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
    let host = crate::runtime_provisions::plan_host_provisions(
        admitted
            .providers
            .installation
            .as_ref()
            .map(|installation| installation.terminal_selection()),
        admitted
            .providers
            .settlements
            .iter()
            .map(
                |settlement| crate::runtime_provisions::TerminalRuntimeHostProvision {
                    boundary: settlement.boundary,
                    realization: settlement.realization.clone(),
                },
            )
            .collect(),
        admitted.input.optimization_input().runtime_input(),
        admission.providers.selected_provider_plans,
        admission.providers.external_binding_rows,
        crate::runtime_provisions::NativeBindingRuntimeInputs {
            bindings: admission.providers.native_bindings,
            calling: admission.providers.installed_calling,
        },
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let runtime = crate::runtime_provisions::TerminalCallableRuntimeProvisions::admit(
        artifact.view(),
        admitted.input.optimization_input().runtime_input(),
        callable_entry,
        admission.providers.profile,
        host,
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let optimized_target = crate::compile_body::plan_native_body(
        admitted.input,
        target_lowering::OptimizedTargetLoweringRequest {
            target: admission.providers.target(),
            settlements: &admitted.providers.settlements,
            installation: admitted.providers.installation.take(),
        },
        selections,
        budget,
        &observation,
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
    pub entry_parameters: &'a [provider_correspondence::callbacks::NativeCallbackEntryParameter],
}
impl<'a> From<&'a TerminalCallbackOccurrenceProposal> for PrivateEntryContract<'a> {
    fn from(row: &'a TerminalCallbackOccurrenceProposal) -> Self {
        let body = row.callback_thunk_artifact();
        Self {
            terminal_operation: row.terminal_operation(),
            placement_index: row.placement_index(),
            callback_function: row.callback_thunk_identity(),
            psi: body.artifact().view().identity(),
            entry: body.entry(),
            boundary_entry_plan: row.boundary_entry_plan(),
            parameter_formals: body.parameter_formals(),
            entry_parameters: row.entry_parameters(),
        }
    }
}

pub struct PlannedNativeProgram<'contract> {
    pub bodies: PlannedProgramBodies<'contract>,
    pub providers: provider_admission::AdmittedNativeProviders<'contract>,
    pub boundary_application_coverage:
        Option<boundary_applications::TerminalBoundaryApplicationCoverage>,
}

pub fn plan_program<'contract>(
    mut admitted: artifact_lowering::AdmittedNativeRealization<'contract>,
    request: &NativeProgramPlanningRequest<'_, 'contract>,
    observation: &crate::observations::NativePipelineObservation,
) -> Result<PlannedNativeProgram<'contract>, Vec<Diagnostic>> {
    let root = crate::compile_body::plan_native_body(
        admitted.input,
        target_lowering::OptimizedTargetLoweringRequest {
            target: request.admission.providers.target(),
            settlements: &admitted.providers.settlements,
            installation: admitted.providers.installation.take(),
        },
        request.optimization_selections,
        request.budget,
        observation,
        request.retain_native_stage_inputs,
    )
    .map_err(|error| match error {
        NativeBodyPlanningError::AbstractOptimization(error) => {
            realization_error("canonical abstract optimization", error)
        }
        NativeBodyPlanningError::Target(error) => realization_error("target lowering", error),
    })?;
    let bodies = PlannedProgramBodies::from_root(
        root,
        request,
        admitted.providers.program_provider_context(),
    )?;
    Ok(PlannedNativeProgram {
        bodies,
        providers: admitted.providers,
        boundary_application_coverage: admitted.boundary_application_coverage,
    })
}

pub struct PlannedProgramBodies<'contract> {
    root: target_lowering::NativeBodyPlan,
    placement_count: usize,
    bodies: Vec<(PrivateEntryContract<'contract>, CallableTargetProduct)>,
}
impl<'contract> PlannedProgramBodies<'contract> {
    pub fn into_parts(
        self,
    ) -> (
        target_lowering::NativeBodyPlan,
        usize,
        Vec<(PrivateEntryContract<'contract>, CallableTargetProduct)>,
    ) {
        (self.root, self.placement_count, self.bodies)
    }

    fn from_root(
        root: target_lowering::NativeBodyPlan,
        request: &NativeProgramPlanningRequest<'_, 'contract>,
        program_provider_context: &Arc<
            provider_admission::AdmittedProgramProviderContext,
        >,
    ) -> Result<Self, Vec<Diagnostic>> {
        let occurrences = &request.proposal.occurrences().callbacks;
        let placement_count = occurrences.len();
        let mut demand = PrivateBodyDemand::new(occurrences)?;
        demand.enqueue(&root)?;
        let mut bodies = Vec::with_capacity(demand.pending.len());
        while let Some(&index) = demand.pending.get(bodies.len()) {
            let occurrence = &occurrences[index];
            let target = plan_callback_target(request, occurrence, program_provider_context)?;
            demand.enqueue(&target.target)?;
            bodies.push((PrivateEntryContract::from(occurrence), target));
        }
        Ok(Self {
            root,
            placement_count,
            bodies,
        })
    }
}

/// The proposal has one occurrence per original placement. Each retained body
/// names its direct callees after target lowering, including installed gates.
struct PrivateBodyDemand<'a> {
    occurrences: &'a [TerminalCallbackOccurrenceProposal],
    by_placement: Vec<Option<usize>>,
    queued: Vec<bool>,
    pending: Vec<usize>,
}

impl<'a> PrivateBodyDemand<'a> {
    fn new(occurrences: &'a [TerminalCallbackOccurrenceProposal]) -> Result<Self, Vec<Diagnostic>> {
        let mut by_placement = vec![None; occurrences.len()];
        for (index, row) in occurrences.iter().enumerate() {
            let slot = by_placement.get_mut(row.placement_index()).ok_or_else(|| {
                realization_error(
                    "callback body closure",
                    "placement falls outside complete original roster",
                )
            })?;
            if slot.replace(index).is_some() {
                return Err(realization_error(
                    "callback body closure",
                    "original callback placement is duplicated",
                ));
            }
        }
        Ok(Self {
            occurrences,
            by_placement,
            queued: vec![false; occurrences.len()],
            pending: Vec::new(),
        })
    }

    fn enqueue(&mut self, body: &target_lowering::NativeBodyPlan) -> Result<(), Vec<Diagnostic>> {
        let psi = body.execution_plan().plan().psi;
        let admission = &body.semantic_facts().admission;
        let callbacks = admission.native_callbacks();
        let interrupts = admission.installed_interrupts();
        if callbacks.psi() != psi || interrupts.psi() != psi {
            return Err(realization_error(
                "callback body closure",
                "admitted private entries belong to another program",
            ));
        }
        let first_new = self.pending.len();
        for &identity in body.private_references() {
            let index = identity
                .callback_thunk_placement_index()
                .and_then(|placement| self.by_placement.get(placement).copied().flatten())
                .ok_or_else(|| {
                    realization_error(
                        "callback body closure",
                        "referenced private function has no original placement",
                    )
                })?;
            let row = &self.occurrences[index];
            let matches = |operation, placement, function| {
                operation == row.terminal_operation()
                    && placement == row.placement_index()
                    && function == identity
            };
            if identity.caller() != psi
                || identity != row.callback_thunk_identity()
                || !(callbacks
                    .callbacks()
                    .iter()
                    .any(|c| matches(c.terminal_operation, c.placement_index, c.callback_function))
                    || interrupts
                        .entries()
                        .iter()
                        .any(|e| matches(e.operation, e.placement_index, e.callback_function)))
            {
                return Err(realization_error(
                    "callback body closure",
                    "referenced private function differs from its admitted occurrence",
                ));
            }
            if !std::mem::replace(&mut self.queued[index], true) {
                self.pending.push(index);
            }
        }
        // Keep the producer's occurrence order for siblings, including when a
        // proposal lists its placements out of numeric order.
        self.pending[first_new..].sort_unstable();
        Ok(())
    }
}

#[cfg(test)]
pub fn inspect_private_body_demand(
    occurrences: &[TerminalCallbackOccurrenceProposal],
    bodies: &[&target_lowering::NativeBodyPlan],
) -> Result<Vec<usize>, Vec<Diagnostic>> {
    let mut demand = PrivateBodyDemand::new(occurrences)?;
    for body in bodies {
        demand.enqueue(body)?;
    }
    Ok(demand.pending)
}

fn plan_callback_target(
    request: &NativeProgramPlanningRequest<'_, '_>,
    occurrence: &TerminalCallbackOccurrenceProposal,
    program_provider_context: &Arc<
        provider_admission::AdmittedProgramProviderContext,
    >,
) -> Result<CallableTargetProduct, Vec<Diagnostic>> {
    let body = occurrence.callback_thunk_artifact();
    let artifact = body.artifact();
    let admission = artifact_lowering::NativeAdmissionRequest {
        private_entry_ram: &[],
        entry: terminal_psi::TargetMachineCallable::Machine(body.entry()),
        providers: provider_admission::ProviderAdmissionRequest {
            program_provider_context: Some(program_provider_context),
            ..request.admission.providers
        },
        ..*request.admission
    };
    let callable = provider_realization::native_execution::BoundNativeCallable::prepare(
        request.proposal, request.execution, artifact, admission.entry,
    )?;
    let (input, boundary_coverage) = artifact_lowering::lower_native_callable(
        callable,
        request
            .scalar_applications
            .map(|completed| artifact_lowering::NativeScalarApplications { completed }),
    )?;
    let admission = artifact_lowering::NativeAdmissionRequest {
        boundary_application_coverage: Some(&boundary_coverage),
        ..admission
    };
    let admitted = admit_prepared_native_realization(input, artifact, &admission)?;
    plan_callable(
        admitted,
        admission,
        request.optimization_selections,
        request.budget,
        artifact,
        request.retain_native_stage_inputs,
    )
}
