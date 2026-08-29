use omega_assigned_target_operations::AssignedOperationPlan;

use crate::{ValidatedOptimizedTargetOperations, ValidatedTargetRegisterEnvironment};

use super::{OptimizedAssignmentCustodyError, StagedOptimizedAssignmentCustodyReceipt};

/// Reconstruct exact cross-stage custody without trusting assignment to stamp
/// its own receipt. Physical-home legality remains outside this check.
pub fn validate_optimized_assignment_custody(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
    assigned: &AssignedOperationPlan,
) -> Result<StagedOptimizedAssignmentCustodyReceipt, OptimizedAssignmentCustodyError> {
    let target = optimized_target.target_operations();
    if optimized_target.target() != target.target {
        return Err(OptimizedAssignmentCustodyError::OptimizedTargetMismatch);
    }
    if register_environment.target() != target.target {
        return Err(OptimizedAssignmentCustodyError::RegisterEnvironmentTargetMismatch);
    }
    if target.psi != assigned.psi {
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
        psi: target.psi,
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
