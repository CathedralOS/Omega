use omega_abstract_operations_to_target_operations::LoweringError;
use omega_assigned_target_operations::AssignedOperationPlan;
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::AssignmentError;
use psi_core::MachineId;
use psi_terminal::TerminalPsiIdentity;

use crate::{
    TargetRegisterEnvironmentValidationError, ValidatedOptimizedTargetOperations,
    ValidatedTargetRegisterEnvironment,
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
    pub(super) optimized_target: ValidatedOptimizedTargetOperations,
    pub(super) register_environment: ValidatedTargetRegisterEnvironment,
    pub(super) assigned: AssignedOperationPlan,
    pub(super) custody: StagedOptimizedAssignmentCustodyReceipt,
}

impl StagedOptimizedAssignedOperations {
    pub const fn optimized_target(&self) -> &ValidatedOptimizedTargetOperations {
        &self.optimized_target
    }

    pub const fn assigned(&self) -> &AssignedOperationPlan {
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
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub(super) function_count: usize,
}

impl StagedOptimizedAssignmentCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
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
