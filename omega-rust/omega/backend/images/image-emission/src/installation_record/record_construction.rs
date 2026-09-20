//! Building an installation record from an emitted image, its evidence and
//! selected provider plans, and deriving the installed stack demand.

use crate::ExecutableImage;
use crate::installation_record::codec::fingerprint_codec::fingerprint_image;
use crate::installation_record::record_shape::validate_record_shape;
use crate::installation_record::record_validation::{
    installed_compiler_private_function, installed_dynamic_calls,
    installed_dynamic_conformance_tables, installed_dynamic_parameter_calls,
    installed_foreign_call_stacks, installed_forwarded_dynamic_descriptor_adapters,
    installed_forwarded_dynamic_descriptor_calls, installed_forwarded_dynamic_descriptor_tables,
    installed_forwarded_dynamic_parameter_calls, installed_image_sections,
    installed_internal_unit_calls, installed_stored_dynamic_calls,
};
use crate::installation_record::{
    InstallationError, InstallationRecord, InstalledComponentProgress, InstalledFunction,
    InstalledInternalUnitScalarCall, InstalledStructuralReturn, SelectedProviderPlanReportIdentity,
    validate_installation_record,
};
use semantic_vocabulary::{MachineId, ProfileDecisionId};
use std::num::NonZeroU64;

/// Build the canonical installation record for an emitted image.
///
/// This convenience path succeeds only when the image has no provider-backed
/// settlements. Effectful images must use the admission-bearing constructor.
pub fn build_installation_record(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
) -> Result<InstallationRecord, InstallationError> {
    build_installation_record_with_provider_executions(
        image,
        profile_decision,
        std::iter::empty::<&dyn installation_evidence::ProviderExecutionEvidence>(),
    )
}

/// Build an installation record from the same ledger-owned provider
/// executions consumed by effectful terminal lowering.
///
/// The execution closure must match the image's retained settlement evidence
/// exactly. Numeric provider-plan identities are derived here and cannot be
/// supplied independently by the caller.
pub fn build_installation_record_with_provider_executions<'execution, Execution>(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
    provider_executions: impl IntoIterator<Item = &'execution Execution>,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
{
    build_installation_record_with_evidence(image, profile_decision, provider_executions, None)
}

/// Build the terminal installation record while committing one already
/// admitted component-progress closure into artifact identity. The evidence
/// trait exposes report identities only; runtime publication must still
/// retain and replay the opaque acceptance owned by orchestration.
pub fn build_installation_record_with_evidence<'execution, Execution>(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
    provider_executions: impl IntoIterator<Item = &'execution Execution>,
    component_progress: Option<&dyn installation_evidence::ComponentProgressAcceptanceEvidence>,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
{
    let provider_executions = provider_executions.into_iter().collect::<Vec<_>>();
    let mut selected_provider_plans = provider_executions
        .iter()
        .map(|execution| execution.provider_plan_report_identity())
        .collect::<Vec<_>>();
    selected_provider_plans.sort_unstable();
    selected_provider_plans.dedup();
    build_installation_record_with_selected_provider_plans_and_evidence(
        image,
        profile_decision,
        selected_provider_plans,
        provider_executions,
        component_progress,
        boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
    )
}

/// Build a terminal installation record from the complete selected provider
/// plan closure plus the admitted executions actually used by the image.
///
/// Selected plans need not all execute in this image, but every execution must
/// belong to the selected closure and the execution closure must still match
/// the image's retained boundary settlements exactly.
///
/// `boundary_opaque_applications` is the artifact's retained by-value opaque
/// custody — the boundary application coverage's `opaque_applications`, or the
/// canonical empty set when the artifact carries no coverage. Bind rejects a
/// record whose claimed custody disagrees with the installed artifact's.
pub fn build_installation_record_with_selected_provider_plans_and_evidence<'execution, Execution>(
    image: &ExecutableImage,
    profile_decision: ProfileDecisionId,
    selected_provider_plans: impl IntoIterator<Item = u64>,
    provider_executions: impl IntoIterator<Item = &'execution Execution>,
    component_progress: Option<&dyn installation_evidence::ComponentProgressAcceptanceEvidence>,
    boundary_opaque_applications: boundary_applications::BoundaryOpaqueRepresentationApplications,
) -> Result<InstallationRecord, InstallationError>
where
    Execution: installation_evidence::ProviderExecutionEvidence + ?Sized + 'execution,
{
    let compiler_text_validation = image
        .output()
        .compiler_text_validation
        .ok_or(InstallationError::MissingCompilerTextValidation)?;
    let mut reported_executions = std::collections::BTreeSet::new();
    let mut selected_provider_plan_set = std::collections::BTreeSet::new();
    for identity in selected_provider_plans {
        let identity = SelectedProviderPlanReportIdentity::new(identity)
            .ok_or(InstallationError::ZeroProviderPlan)?;
        if !selected_provider_plan_set.insert(identity) {
            return Err(InstallationError::DuplicateProviderPlan);
        }
    }
    for execution in provider_executions {
        if !reported_executions.insert((
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        )) {
            return Err(InstallationError::DuplicateProviderExecution);
        }
        let execution_plan =
            SelectedProviderPlanReportIdentity::new(execution.provider_plan_report_identity())
                .ok_or(InstallationError::ZeroProviderPlan)?;
        if !selected_provider_plan_set.contains(&execution_plan) {
            return Err(InstallationError::ProviderExecutionOutsideSelectedClosure);
        }
    }
    let required_executions = image
        .boundary_settlements()
        .iter()
        .filter_map(|installed| {
            let machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
                installed.settlement.execution
            else {
                return None;
            };
            Some((
                execution.provider_plan_report_identity,
                execution.provider_execution_report_identity,
                execution.provider_execution_report_fingerprint,
                execution.normalized_root_report_identity,
                execution.boundary_contract_report_fingerprint,
            ))
        })
        .chain(image.foreign_calls().iter().map(|call| {
            let execution = call.provider_execution;
            (
                execution.provider_plan_report_identity,
                execution.provider_execution_report_identity,
                execution.provider_execution_report_fingerprint,
                execution.normalized_root_report_identity,
                execution.boundary_contract_report_fingerprint,
            )
        }))
        .collect::<std::collections::BTreeSet<_>>();
    if reported_executions != required_executions {
        return Err(InstallationError::ProviderExecutionClosureMismatch);
    }
    let component_progress = component_progress
        .map(|acceptance| {
            let manifest = NonZeroU64::new(acceptance.component_progress_manifest_identity())
                .ok_or(InstallationError::ZeroComponentProgressManifestIdentity)?;
            let acceptance = NonZeroU64::new(acceptance.component_progress_acceptance_identity())
                .ok_or(InstallationError::ZeroComponentProgressAcceptanceIdentity)?;
            Ok(InstalledComponentProgress {
                manifest,
                acceptance,
            })
        })
        .transpose()?;
    let record = InstallationRecord {
        psi: image.psi(),
        target: image.target(),
        subsystem: image.subsystem(),
        profile_decision,
        selected_provider_plans: selected_provider_plan_set.into_iter().collect(),
        component_progress,
        functions: image
            .functions()
            .iter()
            .map(|function| InstalledFunction {
                machine: function.machine,
                scalar_abi: function.scalar_abi.clone(),
                mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
                parameter_abi: function.parameter_abi.clone(),
                structural_call_scalar_return: function.structural_call_scalar_return,
                text_offset: function.text_offset,
                byte_count: function.byte_count,
                unit_stack: function.unit_stack,
                scalar_stack: function.scalar_stack,
                unit_call_stacks: function.unit_call_stacks.clone(),
                scalar_call_stacks: function.scalar_call_stacks.clone(),
                foreign_call_stacks: installed_foreign_call_stacks(image, function.machine),
                unit_body: function.unit_affine_cleanup.is_some(),
                unit_parameters: function.unit_parameters.clone(),
                unit_parameter_homes: function.unit_parameter_homes.clone(),
                unit_scalar_homes: function.unit_scalar_homes.clone(),
                unit_integer_constants: function.unit_integer_constants.clone(),
                unit_affine_scalar_records: function.unit_affine_scalar_records.clone(),
                unit_structural_scalar_field_stores: function
                    .unit_structural_scalar_field_stores
                    .clone(),
                unit_write_only_primitive_stores: function.unit_write_only_primitive_stores.clone(),
                scalar_structural_scalar_field_stores: function
                    .scalar_structural_scalar_field_stores
                    .clone(),
                unit_continuations: function.unit_continuations.clone(),
                unit_affine_cleanup: function.unit_affine_cleanup.clone(),
                scalar_affine_cleanup: function.scalar_affine_cleanup.clone(),
                scalar_control_affine_cleanups: function
                    .scalar_control_affine_cleanups
                    .iter()
                    .map(|record| record.cleanup.clone())
                    .collect(),
                scalar_structural_parameters: function.scalar_structural_parameters.clone(),
                scalar_structural_parameter_homes: function
                    .scalar_structural_parameter_homes
                    .clone(),
                attachment: function.attachment,
            })
            .collect(),
        private_functions: image
            .private_functions()
            .iter()
            .map(installed_compiler_private_function)
            .collect::<Result<Vec<_>, _>>()?,
        structural_returns: image
            .functions()
            .iter()
            .filter_map(|function| {
                function
                    .structural_return
                    .clone()
                    .map(|returned| InstalledStructuralReturn {
                        machine: function.machine,
                        returned,
                    })
            })
            .collect(),
        internal_unit_calls: installed_internal_unit_calls(image),
        internal_unit_scalar_calls: image
            .functions()
            .iter()
            .flat_map(|function| {
                function
                    .internal_unit_scalar_calls
                    .iter()
                    .cloned()
                    .map(|custody| InstalledInternalUnitScalarCall {
                        machine: function.machine,
                        text_offset: function.text_offset + custody.code_offset,
                        custody,
                    })
            })
            .collect(),
        dynamic_conformance_tables: installed_dynamic_conformance_tables(image),
        dynamic_calls: installed_dynamic_calls(image)?,
        stored_dynamic_calls: installed_stored_dynamic_calls(image)?,
        forwarded_dynamic_descriptor_adapters: installed_forwarded_dynamic_descriptor_adapters(
            image,
        ),
        forwarded_dynamic_descriptor_tables: installed_forwarded_dynamic_descriptor_tables(image),
        forwarded_dynamic_descriptor_calls: installed_forwarded_dynamic_descriptor_calls(image)?,
        dynamic_parameter_calls: installed_dynamic_parameter_calls(image)?,
        forwarded_dynamic_parameter_calls: installed_forwarded_dynamic_parameter_calls(image)?,
        semantic_code_attribution: image.semantic_code_attribution().to_vec(),
        port_effects: image.port_effects().to_vec(),
        boundary_settlements: image.boundary_settlements().to_vec(),
        boundary_opaque_applications,
        image: fingerprint_image(&image.output().bytes),
        image_sections: installed_image_sections(image),
        compiler_text_validation,
    };
    validate_record_shape(&record)?;
    Ok(record)
}

/// Recompose the exact internal stack closure retained by a canonical
/// installation record. The selected entry is supplied by installed-root
/// realization; external entry-adapter and interrupt-arrival demand remain
/// outside this artifact-owned closure.
pub fn derive_installation_stack_demand(
    record: &InstallationRecord,
    image: &ExecutableImage,
    entry: MachineId,
) -> Result<crate::StackDemand, InstallationStackError> {
    validate_installation_record(record, image)?;
    let functions = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    if !functions.contains_key(&entry) {
        return Err(crate::ObjectError::EntryFunctionMissing(entry).into());
    }
    let mut active = std::collections::BTreeSet::new();
    let mut memoized = std::collections::BTreeMap::new();
    let mut contributing_machines = std::collections::BTreeSet::new();
    let mut admitted_contribution_report_identities = std::collections::BTreeSet::new();
    let mut admitted_contribution_commitments = std::collections::BTreeSet::new();
    let ceiling_bytes = derive_installed_stack_peak(
        entry,
        &functions,
        &mut active,
        &mut memoized,
        &mut contributing_machines,
        &mut admitted_contribution_report_identities,
        &mut admitted_contribution_commitments,
    )?;
    Ok(crate::StackDemand {
        psi: record.psi,
        target: record.target,
        entry,
        ceiling_bytes,
        stack_alignment: 16,
        contributing_machines,
        admitted_contribution_report_identities,
        admitted_contribution_commitments,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallationStackError {
    Installation(InstallationError),
    Stack(crate::ObjectError),
}

impl From<InstallationError> for InstallationStackError {
    fn from(error: InstallationError) -> Self {
        Self::Installation(error)
    }
}

impl From<crate::ObjectError> for InstallationStackError {
    fn from(error: crate::ObjectError) -> Self {
        Self::Stack(error)
    }
}

impl std::fmt::Display for InstallationStackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for InstallationStackError {}

fn derive_installed_stack_peak(
    machine: MachineId,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
    active: &mut std::collections::BTreeSet<MachineId>,
    memoized: &mut std::collections::BTreeMap<MachineId, u64>,
    contributing_machines: &mut std::collections::BTreeSet<MachineId>,
    admitted_contribution_report_identities: &mut std::collections::BTreeSet<
        task_plans::AdmittedStackContributionReportId,
    >,
    admitted_contribution_commitments: &mut std::collections::BTreeSet<
        task_plans::SameStackContributionCommitment,
    >,
) -> Result<u64, crate::ObjectError> {
    if let Some(peak) = memoized.get(&machine) {
        contributing_machines.insert(machine);
        return Ok(*peak);
    }
    if !active.insert(machine) {
        return Err(crate::ObjectError::TerminalStackCycle(machine));
    }
    contributing_machines.insert(machine);
    let function =
        functions
            .get(&machine)
            .copied()
            .ok_or(crate::ObjectError::UnknownInternalCallTarget {
                caller: machine,
                target: machine,
            })?;
    let mut peak = match (function.unit_stack, function.scalar_stack) {
        (Some(_), Some(_)) => {
            return Err(crate::ObjectError::ConflictingTerminalStackEvidence(
                machine,
            ));
        }
        (Some(stack), None) => u64::from(stack.local_peak_bytes),
        (None, Some(stack)) => u64::from(stack.local_peak_bytes),
        (None, None) => {
            return Err(crate::ObjectError::UnaccountedTerminalStack(machine));
        }
    };
    for (owner, target, caller_live_bytes) in function
        .unit_call_stacks
        .iter()
        .map(|call| (call.owner, call.target, call.caller_live_bytes))
        .chain(
            function
                .scalar_call_stacks
                .iter()
                .map(|call| (call.owner, call.target, call.caller_live_bytes)),
        )
    {
        let callee_peak = derive_installed_stack_peak(
            target,
            functions,
            active,
            memoized,
            contributing_machines,
            admitted_contribution_report_identities,
            admitted_contribution_commitments,
        )?;
        let composed = u64::from(caller_live_bytes)
            .checked_add(callee_peak)
            .ok_or(crate::ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner,
            })?;
        peak = peak.max(composed);
    }
    for call in &function.foreign_call_stacks {
        let composed = u64::from(call.caller_live_bytes)
            .checked_add(call.contribution_bytes)
            .ok_or(crate::ObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            })?;
        peak = peak.max(composed);
        admitted_contribution_report_identities.insert(call.contribution_report_identity);
        admitted_contribution_commitments.insert(call.contribution_commitment);
    }
    active.remove(&machine);
    memoized.insert(machine, peak);
    Ok(peak)
}
