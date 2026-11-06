use optimization_core::OptimizationExecutionPhase;
use optimization_unit::OptimizedAbstractProgram;
use provider_admission::{AdmittedLoweringFacts, AdmittedProviderInstallation};
use std::sync::Arc;
use target::NativeTarget;
use target_operations::TargetOperationPlan;

use crate::{AdmittedBoundarySettlement, LoweringError};

/// Target lowering retaining the optimized abstract program, the admitted
/// facts of the input it was admitted from, and its admitted provider
/// installation beside the produced program. The current program is
/// shared immutable representation data. Borrowing or retaining it does not
/// grant the admission held by this private constructor.
#[derive(Debug)]
pub struct ValidatedOptimizedTargetOperations {
    pub(super) optimized: OptimizedAbstractProgram,
    lowering_facts: AdmittedLoweringFacts,
    current_program: Arc<TargetOperationPlan>,
    installed_stack_requirements: Vec<installation_evidence::InstalledInterruptStackRequirement>,
    installed_entry_execution: Option<provider_admission::AdmittedInstalledEntryExecution>,
    pub(super) provider_installation: Option<Box<AdmittedProviderInstallation>>,
}

impl ValidatedOptimizedTargetOperations {
    /// Keep privileged termination local to this exact installed private body.
    /// Final entry composition must still prove its parent installation.
    pub fn with_installed_entry_execution(
        mut self,
        execution: provider_admission::AdmittedInstalledEntryExecution,
    ) -> Result<Self, &'static str> {
        let entry = execution.entry().entry();
        if self.installed_entry_execution.is_some()
            || entry.psi() != self.current_program.psi
            || entry.target() != self.current_program.target
            || entry.machine() != self.current_program.entry
        {
            return Err("installed execution differs from the current private object");
        }
        self.installed_entry_execution = Some(execution);
        Ok(self)
    }

    pub fn installed_entry_execution(
        &self,
    ) -> Option<&provider_admission::AdmittedInstalledEntryExecution> {
        self.installed_entry_execution.as_ref()
    }

    /// Physical selection may use these capacities only conditionally. Final
    /// object publication recomputes each one from its exact linked body and
    /// arrival bridge; this method grants no installation or stack authority.
    pub fn with_installed_stack_requirements(
        mut self,
        mut requirements: Vec<installation_evidence::InstalledInterruptStackRequirement>,
    ) -> Result<Self, &'static str> {
        let cohort = self.lowering_facts.installed_interrupts();
        let mut entries: Vec<_> = cohort
            .entries()
            .iter()
            .map(|entry| (entry.operation, entry.placement_index))
            .collect();
        entries.sort_unstable();
        requirements.sort_unstable_by_key(|row| (row.operation, row.placement_index));
        if entries.windows(2).any(|pair| pair[0] == pair[1])
            || requirements.windows(2).any(|pair| {
                (pair[0].operation, pair[0].placement_index)
                    == (pair[1].operation, pair[1].placement_index)
            })
        {
            return Err("installed stack requirement or admitted placement is duplicated");
        }
        for requirement in &requirements {
            if requirement.psi != self.current_program.psi
                || requirement.target != self.current_program.target
                || requirement.required_bytes().is_none()
                || requirement.alignment == 0
                || !requirement.alignment.is_power_of_two()
                || entries
                    .binary_search(&(requirement.operation, requirement.placement_index))
                    .is_err()
            {
                return Err("installed stack requirement has no matching admitted entry");
            }
        }
        self.installed_stack_requirements = requirements;
        Ok(self)
    }

    pub fn installed_stack_requirements(
        &self,
    ) -> &[installation_evidence::InstalledInterruptStackRequirement] {
        &self.installed_stack_requirements
    }

    /// The target producer's source-free geometry for borrowed boundary inputs.
    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        &self.current_program.borrowed_storage_layouts
    }
    pub const fn optimized(&self) -> &OptimizedAbstractProgram {
        &self.optimized
    }

    /// The admitted facts of the input the optimized program was admitted
    /// from: module, program entry, assumed bindings, target machine
    /// selections and the sealed callback cohort.
    pub const fn lowering_facts(&self) -> &AdmittedLoweringFacts {
        &self.lowering_facts
    }

    /// The admitted boundary facts this plan's native call origins rejoin
    /// to, from the verified input its optimized source was admitted with.
    pub fn boundary_origins(&self) -> target_operations::BoundaryOriginFacts<'_> {
        let input = &self.lowering_facts;
        target_operations::BoundaryOriginFacts {
            program_entry: input.program_entry().map(|call| {
                target_operations::ProgramEntryOrigin {
                    boundary: call.boundary(),
                    entry: call.entry(),
                    source_signature: call.source_signature(),
                }
            }),
            assumed_bindings: input.assumed_bindings(),
            target_machine_selections: input.target_machine_selections(),
            module: input.module(),
        }
    }

    pub fn target(&self) -> NativeTarget {
        self.current_program.target
    }

    pub fn target_operations(&self) -> &TargetOperationPlan {
        &self.current_program
    }

    /// The original current program, not a snapshot recovered from replay inputs.
    /// This owner exposes raw data only; it cannot reconstruct this admission.
    pub fn shared_program(&self) -> Arc<TargetOperationPlan> {
        Arc::clone(&self.current_program)
    }

    /// Borrow the exact provider installation retained beside any target
    /// operations it authorized. It cannot detach from the joined custody.
    pub fn provider_installation(&self) -> Option<&AdmittedProviderInstallation> {
        self.provider_installation.as_deref()
    }
}

/// The admitted settlements an optimized target lowering consumes beyond the
/// validated plan and its target.
/// Callback authority belongs to the original verified input, never this
/// independently constructed request. A cohort from another artifact, entry,
/// provider selection or target cannot be substituted through this entrance.
///
/// ```compile_fail
/// fn inject(request: &mut abstract_operations_to_target_operations::OptimizedTargetLoweringRequest<'_>,
///     foreign: &provider_admission::AdmittedLoweringFacts) {
///     request.native_callbacks = foreign.native_callbacks().callbacks();
/// }
/// ```
pub struct OptimizedTargetLoweringRequest<'a> {
    pub target: NativeTarget,
    pub settlements: &'a [AdmittedBoundarySettlement<'a>],
    /// The exact installation retained beside the target operations it
    /// authorized.
    pub installation: Option<AdmittedProviderInstallation>,
}

/// One target lowering entrance for identity and selected abstract programs.
/// Native admissions are explicit request data, not a reason to select a
/// different target producer.
///
/// The entrance binds every admitted input as it lowers: the admitted provider
/// executions, the settlement roster, the installation join and the selected
/// call index all run here as the program is produced. Replaying that output is
/// the independent audit's job; this default route does not re-derive what it
/// just lowered.
pub fn lower_optimized_to_target_operations(
    optimized: OptimizedAbstractProgram,
    lowering_facts: AdmittedLoweringFacts,
    request: OptimizedTargetLoweringRequest<'_>,
) -> Result<ValidatedOptimizedTargetOperations, LoweringError> {
    let OptimizedTargetLoweringRequest {
        target,
        settlements,
        installation,
    } = request;
    // Translation is implemented; selected target-operation rewrites are not.
    let phase = OptimizationExecutionPhase::TargetOperations;
    if !optimized.selections().for_phase(phase).is_empty() {
        return Err(LoweringError::UnconsumedOptimizationPhase(phase));
    }
    // The callback cohort must be the one admitted for this exact Terminal
    // program; a cohort admitted for another program cannot be substituted.
    if lowering_facts.native_callbacks().psi() != optimized.plan().psi {
        return Err(LoweringError::ForeignNativeCallbackCohort);
    }
    if lowering_facts.installed_interrupts().psi() != optimized.plan().psi {
        return Err(LoweringError::ForeignInstalledInterruptCohort);
    }
    let installed = installation
        .as_ref()
        .map(|value| value as &dyn installation_evidence::ProviderInstallationEvidence);
    let bindings =
        super::provider_evidence::bind_provider_executions(optimized.plan(), settlements)?;
    let program =
        super::coordination::lower_to_target_operations_with_settlements_and_installation(
            optimized.plan(),
            target,
            &bindings,
            installed,
            lowering_facts.native_callbacks().callbacks(),
            Some(&lowering_facts),
        )?;
    Ok(ValidatedOptimizedTargetOperations {
        optimized,
        lowering_facts,
        current_program: Arc::new(program),
        installed_stack_requirements: Vec::new(),
        installed_entry_execution: None,
        provider_installation: installation.map(Box::new),
    })
}
