use omega_lowering_optimizer::{
    ValidatedOptimizedAbstractPlan, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
};
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations_to_target_operations::{
    AdmittedTerminalBoundarySettlement, LoweringError,
};
use omega_terminal_psi_to_abstract_operations::AdmittedTerminalProviderInstallation;
use omega_terminal_assigned_target_operations::TerminalAssignedOperationPlan;
use omega_terminal_target_operations_to_assigned_target_operations::{
    AssignmentError, assign_registers,
};
use psi_core::MachineId;
use psi_terminal::TerminalPsiIdentity;

use crate::{
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment,
};

/// Current target assignment with the complete optimized lowering custody that
/// authorized it.
///
/// This carrier is deliberately `Staged`, not allocator-validated. The current
/// assignment lane is bounded scratch-register assignment and has no
/// independent liveness/interference verifier. Borrowed access supports the
/// next validator cut without allowing an assigned plan to detach from the
/// optimizer run, ledger, and projection receipt. It grants no machine
/// emission or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedAssignedOperations {
    optimized_target: ValidatedOptimizedTargetOperations,
    register_environment: ValidatedTargetRegisterEnvironment,
    assigned: TerminalAssignedOperationPlan,
    custody: StagedOptimizedAssignmentCustodyReceipt,
}

impl StagedOptimizedAssignedOperations {
    pub const fn optimized_target(&self) -> &ValidatedOptimizedTargetOperations {
        &self.optimized_target
    }

    pub const fn assigned(&self) -> &TerminalAssignedOperationPlan {
        &self.assigned
    }

    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        &self.register_environment
    }

    pub const fn custody(&self) -> StagedOptimizedAssignmentCustodyReceipt {
        self.custody
    }
}

/// Independently reconstructed root/provenance custody for one staged
/// assignment. This receipt does not validate physical home legality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAssignmentCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    function_count: usize,
}

impl StagedOptimizedAssignmentCustodyReceipt {
    pub const fn terminal_psi(self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(self) -> NativeTarget {
        self.target
    }

    pub const fn entry(self) -> MachineId {
        self.entry
    }

    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }

    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }

    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }

    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedAssignmentCustodyError {
    OptimizedTargetMismatch,
    RegisterEnvironmentTargetMismatch,
    TerminalPsiMismatch,
    NativeTargetMismatch,
    EntryMismatch,
    FunctionCountMismatch { expected: usize, actual: usize },
    FunctionMachineMismatch { position: usize },
    FunctionAttachmentMismatch { position: usize },
    FunctionProvenanceMismatch { position: usize },
}

impl std::fmt::Display for OptimizedAssignmentCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid optimized assignment custody: {self:?}")
    }
}

impl std::error::Error for OptimizedAssignmentCustodyError {}

#[derive(Debug)]
pub enum OptimizedAssignmentPipelineError {
    TargetLowering(LoweringError),
    RegisterEnvironment(TargetRegisterEnvironmentValidationError),
    Assignment(AssignmentError),
    Custody(OptimizedAssignmentCustodyError),
}

impl std::fmt::Display for OptimizedAssignmentPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized target assignment failed: {self:?}")
    }
}

impl std::error::Error for OptimizedAssignmentPipelineError {}

/// Assign current physical homes while retaining the exact optimized target
/// carrier and independently checking every stage root and function provenance
/// row copied across the boundary.
pub fn stage_optimized_assignment(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedAssignedOperations, OptimizedAssignmentPipelineError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedAssignmentPipelineError::RegisterEnvironment)?;
    let assigned = assign_registers(optimized_target.target_operations())
        .map_err(OptimizedAssignmentPipelineError::Assignment)?;
    let custody =
        validate_optimized_assignment_custody(&optimized_target, &register_environment, &assigned)
            .map_err(OptimizedAssignmentPipelineError::Custody)?;
    Ok(StagedOptimizedAssignedOperations {
        optimized_target,
        register_environment,
        assigned,
        custody,
    })
}

/// Lower and assign one optimized plan without exposing a bare target plan to
/// compiler orchestration.
pub fn stage_optimized_assignment_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedTerminalBoundarySettlement<'_>],
) -> Result<StagedOptimizedAssignedOperations, OptimizedAssignmentPipelineError> {
    let optimized_target = lower_optimized_to_target_operations_with_provider_executions(
        optimized,
        target,
        settlements,
    )
    .map_err(OptimizedAssignmentPipelineError::TargetLowering)?;
    stage_optimized_assignment(optimized_target)
}

/// Lower and assign one optimized plan with one exact checked-provider
/// installation. This is the installation-bearing form of the same canonical
/// assignment stage, not a second native route.
pub fn stage_optimized_assignment_with_provider_executions_and_installation(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedTerminalBoundarySettlement<'_>],
    installation: AdmittedTerminalProviderInstallation,
) -> Result<StagedOptimizedAssignedOperations, OptimizedAssignmentPipelineError> {
    let optimized_target =
        lower_optimized_to_target_operations_with_provider_executions_and_installation(
            optimized,
            target,
            settlements,
            installation,
        )
        .map_err(OptimizedAssignmentPipelineError::TargetLowering)?;
    stage_optimized_assignment(optimized_target)
}

/// Reconstruct exact cross-stage custody without trusting assignment to stamp
/// its own receipt. Physical-home legality remains outside this check.
pub fn validate_optimized_assignment_custody(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
    assigned: &TerminalAssignedOperationPlan,
) -> Result<StagedOptimizedAssignmentCustodyReceipt, OptimizedAssignmentCustodyError> {
    let target = optimized_target.target_operations();
    if optimized_target.target() != target.target {
        return Err(OptimizedAssignmentCustodyError::OptimizedTargetMismatch);
    }
    if register_environment.target() != target.target {
        return Err(OptimizedAssignmentCustodyError::RegisterEnvironmentTargetMismatch);
    }
    if target.terminal_psi != assigned.terminal_psi {
        return Err(OptimizedAssignmentCustodyError::TerminalPsiMismatch);
    }
    if target.target != assigned.target {
        return Err(OptimizedAssignmentCustodyError::NativeTargetMismatch);
    }
    if target.entry != assigned.entry {
        return Err(OptimizedAssignmentCustodyError::EntryMismatch);
    }
    if target.functions.len() != assigned.functions.len() {
        return Err(OptimizedAssignmentCustodyError::FunctionCountMismatch {
            expected: target.functions.len(),
            actual: assigned.functions.len(),
        });
    }
    for (position, (target, assigned)) in
        target.functions.iter().zip(&assigned.functions).enumerate()
    {
        if target.machine != assigned.machine {
            return Err(OptimizedAssignmentCustodyError::FunctionMachineMismatch { position });
        }
        if target.attachment != assigned.attachment {
            return Err(OptimizedAssignmentCustodyError::FunctionAttachmentMismatch { position });
        }
        if target.provenance != assigned.provenance {
            return Err(OptimizedAssignmentCustodyError::FunctionProvenanceMismatch { position });
        }
    }
    Ok(StagedOptimizedAssignmentCustodyReceipt {
        terminal_psi: target.terminal_psi,
        target: target.target,
        entry: target.entry,
        optimization: optimized_target.optimized().identity_bundle().identity(),
        projection: optimized_target.optimized().validation().identity(),
        manifest: optimized_target
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity,
        register_environment: register_environment.identity(),
        function_count: target.functions.len(),
    })
}
