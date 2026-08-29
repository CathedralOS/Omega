#![forbid(unsafe_code)]

//! Shared composition from canonical Terminal Psi to a replayed native artifact.
//!
//! This crate is named for its exact input and output. It sequences the
//! verified optimizer, target legalization/assignment, machine emission, and
//! object/image construction; it is not another intermediate representation
//! or an alternate compiler route. It does not own source compilation,
//! component policy, executable installation, or publication.

mod optimized_semantic_wrapper_encoding;
mod optimized_semantic_wrapper_object;

pub use optimized_semantic_wrapper_encoding::{
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding,
    select_optimized_program_storage_semantic_wrapper_encoding,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};
pub use optimized_semantic_wrapper_object::*;

use std::collections::BTreeSet;

use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_effects::provider_plan::ProviderBinding;
use omega_installation_evidence::ProviderExecutionEvidence;
pub use omega_native_artifact::{
    NativeArtifact, NativeArtifactParts, NativeProviderExecution, NativeSelectedProviderPlan,
};
use omega_psi_to_abstract_operations::{
    SelectedProviderAdapter, admit_provider_installation,
    admit_provider_installation_for_optimization,
};
use omega_target_operations::BoundaryRealization;
use psi_checked_trees_to_terminal::{
    CheckedProgramEntryTerminalReceipt, ProducedProgramEntryTerminalArtifact,
};
use psi_diagnostics::Diagnostic;

enum NativeRealizationInput {
    Ordinary(omega_abstract_operations::AbstractOperationPlan),
    ExplicitOptimization(omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput),
}

impl NativeRealizationInput {
    fn plan(&self) -> &omega_abstract_operations::AbstractOperationPlan {
        match self {
            Self::Ordinary(plan) => plan,
            Self::ExplicitOptimization(input) => input.plan(),
        }
    }
}

/// Provider-supplied realization input for one Terminal boundary. The exact
/// requirement comes from admitted execution evidence rather than a caller-
/// authored numeric boundary ID.
#[derive(Debug, Clone, Copy)]
pub struct NativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn ProviderExecutionEvidence,
    pub realization: BoundaryRealization,
}

/// Exact build-owned source-entry custody carried into native realization.
/// This is declaration and calling-contract evidence only: it owns no runtime
/// roots and cannot authorize a physical bootstrap, image, or publication.
#[derive(Debug, Clone, Copy)]
pub struct NativeProgramEntrySettlement<'entry> {
    source: &'entry omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    semantic_boundary_entry_plan: Option<&'entry omega_calling_conventions::BoundaryEntryPlan>,
    storage_entry: Option<&'entry omega_program_entry_plan::SelectedProgramStorageEntryPlan>,
}

impl<'entry> NativeProgramEntrySettlement<'entry> {
    pub const fn new(
        source: &'entry omega_program_entry_plan::SelectedProgramEntrySourceSignature,
        calling_plans: Option<(
            &'entry omega_calling_conventions::BoundaryEntryPlan,
            &'entry omega_program_entry_plan::SelectedProgramStorageEntryPlan,
        )>,
    ) -> Self {
        let (semantic_boundary_entry_plan, storage_entry) = match calling_plans {
            Some((semantic, storage)) => (Some(semantic), Some(storage)),
            None => (None, None),
        };
        Self {
            source,
            semantic_boundary_entry_plan,
            storage_entry,
        }
    }

    pub const fn source(
        self,
    ) -> &'entry omega_program_entry_plan::SelectedProgramEntrySourceSignature {
        self.source
    }

    pub const fn semantic_boundary_entry_plan(
        self,
    ) -> Option<&'entry omega_calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan
    }

    pub const fn storage_entry(
        self,
    ) -> Option<&'entry omega_program_entry_plan::SelectedProgramStorageEntryPlan> {
        self.storage_entry
    }

    fn validate_for_target(self, target: omega_target::NativeTarget) -> Result<(), String> {
        let slot = self.source.target_slot();
        if slot.owner.native_target() != target {
            return Err(format!(
                "selected ProgramEntry target profile `{}` does not own native target {target:?}",
                slot.owner.target_name(),
            ));
        }
        let declares_two_surfaces = slot.boundary_schema.is_some()
            || slot.physical_arrival_requirement.is_some()
            || slot.physical_contract_package.is_some()
            || slot.physical_calling_convention.is_some()
            || slot.semantic_calling_convention.is_some();
        match (
            declares_two_surfaces,
            self.semantic_boundary_entry_plan,
            self.storage_entry,
        ) {
            (false, None, None) => Ok(()),
            (true, Some(semantic), Some(storage)) => {
                validate_paired_calling_plans(self.source, semantic, storage)
            }
            _ => Err(
                "selected ProgramEntry lost its exact paired semantic/physical calling-plan custody"
                    .into(),
            ),
        }
    }
}

fn validate_paired_calling_plans(
    source: &omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    semantic: &omega_calling_conventions::BoundaryEntryPlan,
    storage: &omega_program_entry_plan::SelectedProgramStorageEntryPlan,
) -> Result<(), String> {
    let slot = source.target_slot();
    let (Some(expected_semantic), Some(expected_physical)) = (
        slot.semantic_calling_convention,
        slot.physical_calling_convention,
    ) else {
        return Err(
            "selected ProgramEntry has an incomplete two-surface calling declaration".into(),
        );
    };
    let expected_policy = |convention| match convention {
        omega_target::ProgramEntryCallingConvention::MicrosoftX64 => {
            omega_calling_conventions::CallingPolicy::MicrosoftX64
        }
    };
    if storage.target_slot() != slot || semantic.call.policy != expected_policy(expected_semantic) {
        return Err(
            "selected ProgramEntry semantic calling plan drifted from its target slot".into(),
        );
    }
    let signature = omega_calling_conventions::CallSignature {
        parameters: source
            .visible_parameters()
            .iter()
            .map(|parameter| parameter.value_shape())
            .collect(),
        result: None,
    };
    let validated_semantic =
        omega_calling_conventions::validate_boundary_entry_plan(semantic.clone(), &signature)
            .map_err(|error| format!("selected ProgramEntry semantic plan is invalid: {error}"))?;
    let matching_methods = storage
        .schema()
        .methods
        .iter()
        .filter(|method| method.requirement_identity == storage.requirement_identity())
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(
            "selected ProgramEntry storage plan lost its unique semantic requirement".into(),
        );
    };
    if method.calling_plan_fingerprint != Some(validated_semantic.contract_fingerprint())
        || method.parameter_type_identities
            != source
                .visible_parameters()
                .iter()
                .map(|parameter| parameter.normalized_type_identity().to_owned())
                .collect::<Vec<_>>()
        || method.has_result
        || method.result_type_identity.is_some()
    {
        return Err(
            "selected ProgramEntry semantic plan is not paired with its source signature".into(),
        );
    }
    let physical = storage
        .physical_contract()
        .ok_or_else(|| "selected ProgramEntry lost its physical calling contract".to_owned())?;
    let physical_plan = physical.boundary_entry_plan();
    let physical_signature = omega_calling_conventions::CallSignature {
        parameters: physical_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: physical_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let validated_physical = omega_calling_conventions::validate_boundary_entry_plan(
        physical_plan.clone(),
        &physical_signature,
    )
    .map_err(|error| format!("selected ProgramEntry physical plan is invalid: {error}"))?;
    if physical.target_slot() != slot
        || physical.requirement_identity() != slot.physical_arrival_requirement.unwrap_or_default()
        || physical_plan.call.policy != expected_policy(expected_physical)
        || validated_physical.contract_fingerprint() != physical.calling_plan_fingerprint()
    {
        return Err("selected ProgramEntry physical plan drifted from its target contract".into());
    }
    Ok(())
}

/// Owned, independently replayed source-entry settlement for one canonical
/// Terminal artifact. It remains declaration and calling-contract custody;
/// it grants no semantic wrapper, physical process entry, image installation,
/// or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNativeProgramEntrySettlement {
    checked_entry: CheckedProgramEntryTerminalReceipt,
    target: omega_target::NativeTarget,
    source: omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    semantic_boundary_entry_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    storage_entry: Option<omega_program_entry_plan::SelectedProgramStorageEntryPlan>,
}

impl ValidatedNativeProgramEntrySettlement {
    pub const fn checked_entry(&self) -> &CheckedProgramEntryTerminalReceipt {
        &self.checked_entry
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn source(&self) -> &omega_program_entry_plan::SelectedProgramEntrySourceSignature {
        &self.source
    }

    pub const fn semantic_boundary_entry_plan(
        &self,
    ) -> Option<&omega_calling_conventions::BoundaryEntryPlan> {
        self.semantic_boundary_entry_plan.as_ref()
    }

    pub const fn storage_entry(
        &self,
    ) -> Option<&omega_program_entry_plan::SelectedProgramStorageEntryPlan> {
        self.storage_entry.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeProgramEntrySettlementError {
    TargetDrift,
    CallingPlanPairingDrift,
    SourceSignatureSubstitution,
    SourceMachineSubstitution,
    CanonicalArtifactReplay(String),
    TerminalPsiSubstitution,
    TerminalEntrySubstitution,
    TerminalEntryMultiplicity(usize),
}

impl std::fmt::Display for NativeProgramEntrySettlementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeProgramEntrySettlementError {}

/// Independently replay the complete source-signature, target, calling-plan,
/// Terminal-Psi, and entry-identity join without invoking the Psi receipt
/// producer.
pub fn validate_native_program_entry_settlement(
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    checked_entry: &CheckedProgramEntryTerminalReceipt,
    program_entry: NativeProgramEntrySettlement<'_>,
    target: omega_target::NativeTarget,
) -> Result<ValidatedNativeProgramEntrySettlement, NativeProgramEntrySettlementError> {
    let slot = program_entry.source.target_slot();
    if slot.owner.native_target() != target {
        return Err(NativeProgramEntrySettlementError::TargetDrift);
    }
    program_entry
        .validate_for_target(target)
        .map_err(|_| NativeProgramEntrySettlementError::CallingPlanPairingDrift)?;
    if checked_entry.source_signature_identity() != program_entry.source.identity().bytes() {
        return Err(NativeProgramEntrySettlementError::SourceSignatureSubstitution);
    }
    if checked_entry.source_machine_name() != program_entry.source.machine_name() {
        return Err(NativeProgramEntrySettlementError::SourceMachineSubstitution);
    }
    artifact.validate().map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let psi = psi_terminal_codec::terminal_psi_identity(&module).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    if psi != checked_entry.terminal_psi_identity()
        || artifact.manifest().semantic() != checked_entry.terminal_psi_identity()
    {
        return Err(NativeProgramEntrySettlementError::TerminalPsiSubstitution);
    }
    if module.entry != checked_entry.terminal_entry() {
        return Err(NativeProgramEntrySettlementError::TerminalEntrySubstitution);
    }
    let entry_count = module
        .machines
        .iter()
        .filter(|machine| machine.id == checked_entry.terminal_entry())
        .count();
    if entry_count != 1 {
        return Err(NativeProgramEntrySettlementError::TerminalEntryMultiplicity(entry_count));
    }
    Ok(ValidatedNativeProgramEntrySettlement {
        checked_entry: checked_entry.clone(),
        target,
        source: program_entry.source.clone(),
        semantic_boundary_entry_plan: program_entry.semantic_boundary_entry_plan.cloned(),
        storage_entry: program_entry.storage_entry.cloned(),
    })
}

/// Complete build-owned inputs for one target-native realization. Keeping
/// these coupled prevents callers from accidentally carrying entry, target,
/// optimization, and provider custody through separate positional channels.
pub struct NativeRealizationRequest<'request> {
    pub target: omega_target::NativeTarget,
    pub subsystem: u16,
    pub profile: &'request psi_proof_admission::AdmissionProfile,
    pub program_entry: NativeProgramEntrySettlement<'request>,
    pub optimization_selections: &'request omega_optimization_core::OptimizationSelections,
    pub selected_provider_plans: &'request omega_effects::SelectedProviderPlanFacts,
    pub settlements: &'request [NativeProviderSettlement<'request>],
}

/// Compatibility-preserving result for the receipt-requiring native path.
#[derive(Debug)]
pub struct SettledNativeArtifact {
    artifact: NativeArtifact,
    program_entry: ValidatedNativeProgramEntrySettlement,
}

impl SettledNativeArtifact {
    pub const fn artifact(&self) -> &NativeArtifact {
        &self.artifact
    }

    pub const fn program_entry(&self) -> &ValidatedNativeProgramEntrySettlement {
        &self.program_entry
    }

    pub fn into_parts(self) -> (NativeArtifact, ValidatedNativeProgramEntrySettlement) {
        (self.artifact, self.program_entry)
    }
}

/// Realize a receipt-coupled checked `ProgramEntry` artifact and return its
/// independently validated, owned native settlement alongside the ordinary
/// authority-free native artifact.
pub fn realize_program_entry_native_artifact(
    produced: ProducedProgramEntryTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<SettledNativeArtifact, Vec<Diagnostic>> {
    let (artifact, checked_entry) = produced.into_parts();
    let program_entry = validate_native_program_entry_settlement(
        &artifact,
        &checked_entry,
        request.program_entry,
        request.target,
    )
    .map_err(|error| realization_error("checked ProgramEntry settlement", error))?;
    let artifact = realize_native_artifact(artifact, request)?;
    Ok(SettledNativeArtifact {
        artifact,
        program_entry,
    })
}

/// Realize one canonical Terminal-Psi artifact into an authority-free target
/// object and executable image while retaining its captured source-entry
/// settlement. Ordinary native compilation and component packaging share this
/// exact operation.
pub fn realize_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    request
        .program_entry
        .validate_for_target(request.target)
        .map_err(|error| realization_error("ProgramEntry custody", error))?;
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let semantic_bytes = artifact.semantic_bytes();
    let proof_bytes = artifact.proof_bytes();
    let realization_input = if request.optimization_selections.is_empty() {
        NativeRealizationInput::Ordinary(
            omega_psi_to_abstract_operations::lower_artifact_sections(
                semantic_bytes,
                proof_bytes,
                request.profile,
            )
            .map_err(|error| realization_error("ordinary artifact lowering", error))?,
        )
    } else {
        NativeRealizationInput::ExplicitOptimization(
            omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                semantic_bytes,
                proof_bytes,
                request.profile,
            )
            .map_err(|error| realization_error("verified optimizer artifact lowering", error))?,
        )
    };

    let mut seen_requirements = BTreeSet::new();
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());
    for settlement in request.settlements {
        let evidence = settlement.provider_execution;
        let requirement = evidence.requirement_identity();
        if !seen_requirements.insert(requirement.to_owned()) {
            return Err(vec![Diagnostic::error(format!(
                "native realization received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let selected_plan = request
            .selected_provider_plans
            .plan_by_identity(evidence.provider_plan())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "native provider execution for `{requirement}` names unselected plan {:#018x}",
                    evidence.provider_plan()
                ))]
            })?;
        if !selected_plan
            .rows
            .iter()
            .any(|row| row.requirement_identity == requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` is absent from selected plan `{}`",
                selected_plan.name
            ))]);
        }
        let matching_boundaries = realization_input
            .plan()
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == requirement)
            .collect::<Vec<_>>();
        let [boundary] = matching_boundaries.as_slice() else {
            return Err(vec![Diagnostic::error(match matching_boundaries.len() {
                0 => format!("native provider execution cites absent requirement `{requirement}`"),
                count => format!(
                    "native requirement `{requirement}` resolves to {count} boundary declarations"
                ),
            })]);
        };
        admitted.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            provider_execution: evidence,
            realization: settlement.realization,
        });
        provider_executions.push(NativeProviderExecution::from_evidence(evidence));
    }
    provider_executions.sort_by(|left, right| {
        (
            left.requirement_identity(),
            left.provider_plan(),
            left.provider_execution_identity(),
        )
            .cmp(&(
                right.requirement_identity(),
                right.provider_plan(),
                right.provider_execution_identity(),
            ))
    });

    let installation_plan = realization_input.plan();
    let provider_installation = if installation_plan.provider_candidates.is_empty() {
        None
    } else {
        let selected =
            project_selected_provider_adapters(request.selected_provider_plans, installation_plan)
                .map_err(|error| {
                    realization_error("selected checked-provider projection", error)
                })?;
        if selected.is_empty() {
            None
        } else {
            Some(
                match &realization_input {
                    NativeRealizationInput::Ordinary(_) => admit_provider_installation(
                        installation_plan,
                        semantic_bytes,
                        proof_bytes,
                        request.profile,
                        &selected,
                    ),
                    NativeRealizationInput::ExplicitOptimization(_) => {
                        admit_provider_installation_for_optimization(
                            installation_plan,
                            semantic_bytes,
                            proof_bytes,
                            request.profile,
                            &selected,
                        )
                    }
                }
                .map_err(|error| {
                    realization_error("checked-provider installation", format!("{error:?}"))
                })?,
            )
        }
    };

    let machine_code = match realization_input {
        NativeRealizationInput::Ordinary(plan) => {
            let target = match provider_installation {
                Some(installation) => omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions_and_installation(
                    &plan,
                    request.target,
                    &admitted,
                    Some(&installation),
                ),
                None => omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
                    &plan,
                    request.target,
                    &admitted,
                ),
            }
            .map_err(|error| realization_error("ordinary target lowering", error))?;
            let assigned =
                omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                    .map_err(|error| realization_error("ordinary physical assignment", error))?;
            omega_machine_emission::emit_machine_code(&assigned)
                .map_err(|error| realization_error("machine-code emission", error))?
        }
        NativeRealizationInput::ExplicitOptimization(input) => {
            let optimization_request = omega_optimization_pipeline::compiler_baseline_request_v1(
                request.optimization_selections,
            )
            .map_err(|error| realization_error("canonical optimization request", error))?;
            let optimized = omega_optimization_pipeline::optimize_verified_psi_input(
                input,
                optimization_request,
            )
            .map_err(|error| realization_error("canonical optimization", error))?;
            let continuation = match provider_installation {
                Some(installation) => omega_optimization_pipeline::stage_optimized_native_continuation_with_provider_executions_and_installation(
                    optimized,
                    request.target,
                    &admitted,
                    installation,
                ),
                None => omega_optimization_pipeline::stage_optimized_native_continuation_with_provider_executions(
                    optimized,
                    request.target,
                    &admitted,
                ),
            }
            .map_err(|error| match error {
                omega_optimization_pipeline::OptimizedNativeContinuationError::CoverageFallbackAssigned(
                    error,
                ) => realization_error("optimized physical assignment", error),
                omega_optimization_pipeline::OptimizedNativeContinuationError::SelectedPhysical(
                    error,
                ) => selected_physical_pipeline_failed(request.optimization_selections, error),
            })?;
            let assigned = match continuation {
                omega_optimization_pipeline::StagedOptimizedNativeContinuation::CoverageFallbackAssigned(
                    assigned,
                ) => assigned,
                omega_optimization_pipeline::StagedOptimizedNativeContinuation::SelectedPhysical(
                    physical,
                ) => {
                    return Err(selected_physical_pipeline_not_publishable(
                        request.optimization_selections,
                        &physical,
                    ));
                }
            };
            omega_machine_emission::emit_machine_code(assigned.assigned())
                .map_err(|error| realization_error("machine-code emission", error))?
        }
    };
    let object = omega_image_emission::build_object_artifact(&machine_code)
        .map_err(|error| realization_error("terminal object construction", error))?;
    let image = omega_image_emission::emit_executable_image(&object, request.subsystem)
        .map_err(|diagnostic| vec![diagnostic])?;

    let mut selected_provider_plan_projections = request
        .selected_provider_plans
        .plans()
        .iter()
        .map(|plan| {
            NativeSelectedProviderPlan::new(
                plan.identity_fingerprint(),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    selected_provider_plan_projections.sort_by_key(NativeSelectedProviderPlan::identity);
    NativeArtifact::from_replayed_parts(NativeArtifactParts {
        target: request.target,
        psi_artifact: artifact,
        object,
        image,
        selected_provider_closure_identity: request.selected_provider_plans.normalized_identity(),
        selected_provider_plans: selected_provider_plan_projections,
        provider_executions,
    })
    .map_err(|error| realization_error("native artifact replay", error))
}

/// Project only checked, in-artifact provider adapters from the selected
/// provider closure. External bindings continue through provider-execution
/// settlements; they must never be reinterpreted as checked Omega machines.
fn project_selected_provider_adapters(
    selected: &omega_effects::SelectedProviderPlanFacts,
    terminal: &omega_abstract_operations::AbstractOperationPlan,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let relevant_requirements = terminal
        .provider_candidates
        .iter()
        .map(|candidate| candidate.requirement_identity.as_str())
        .collect::<BTreeSet<_>>();
    project_selected_provider_adapters_for_requirements(selected, &relevant_requirements)
}

fn project_selected_provider_adapters_for_requirements(
    selected: &omega_effects::SelectedProviderPlanFacts,
    relevant_requirements: &BTreeSet<&str>,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let mut adapters = Vec::new();
    for plan in selected.plans() {
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            if !relevant_requirements.contains(row.requirement_identity.as_str()) {
                continue;
            }
            if plan.provider_type.is_empty() {
                return Err(format!(
                    "selected ProviderPlan `{}` has a checked adapter but no exact provider type identity",
                    plan.name
                ));
            }
            if row.requirement_identity.is_empty() || machine_identity.is_empty() {
                return Err(format!(
                    "selected ProviderPlan `{}` has an incomplete checked-adapter identity",
                    plan.name
                ));
            }
            if *machine_package_identity != plan.origin_package_identity {
                return Err(format!(
                    "selected checked adapter `{machine_identity}` for ProviderPlan `{}` drifted from the plan's exact origin package",
                    plan.name
                ));
            }
            adapters.push(SelectedProviderAdapter {
                requirement_identity: row.requirement_identity.clone(),
                provider_identity: plan.provider_type.clone(),
                machine_identity: machine_identity.clone(),
            });
        }
    }
    adapters.sort_by(|left, right| {
        (
            &left.requirement_identity,
            &left.provider_identity,
            &left.machine_identity,
        )
            .cmp(&(
                &right.requirement_identity,
                &right.provider_identity,
                &right.machine_identity,
            ))
    });
    if let Some(duplicate) = adapters
        .windows(2)
        .find(|rows| rows[0].requirement_identity == rows[1].requirement_identity)
    {
        return Err(format!(
            "selected provider closure projects more than one checked adapter for exact requirement `{}`",
            duplicate[0].requirement_identity
        ));
    }
    Ok(adapters)
}

fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}

fn selected_physical_pipeline_not_publishable(
    selections: &omega_optimization_core::OptimizationSelections,
    physical: &omega_optimization_pipeline::StagedOptimizedVerifiedPhysicalPipeline,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` completed the complete verified optimizer pipeline through selected/physical validation (selection identity {:?}) but cannot yet enter native production: the continuation does not cover baseline frame/exit, executable-image, and publication validation; no alternate compiler route was run and no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
        physical.selections(),
    ))]
}

fn selected_physical_pipeline_failed(
    selections: &omega_optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` failed in the complete verified optimizer pipeline during selected/physical validation: {error}; no alternate compiler route was run and no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
    ))]
}

#[cfg(test)]
mod tests;
