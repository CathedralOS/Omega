/*
`stage_register_allocation` is Omega 05's one entrance. It reads the
allocation-recovery projection of the build's selection, takes the selected
catalog entry (or none), and gets legality under the allocator availability
that entry requires, or all allocatable environment views without a recovery
selection. Omega 04's analysis is reused only when its availability matches;
otherwise allocation derives legality from the unchanged live ranges. It then
runs the procedure the entry's rule names (plain assignment when nothing is selected), each falling
back to runtime spilling under remaining register pressure, and admits the
result.

Omega 04's history does not enter this decision. It arrives as the program's
`SelectedOptimizationRecord` and the allocation manifest carries it as
evidence (omega-rust/pipeline.md#crate-placement-rule;
wiki/spec/build/optimizations.md#phase-and-product-boundaries). Allocation can
still differ by constraints, the selected recovery entry, the analyses Omega 04
handed over and work budgets.
*/

use crate::assignment::recovery::{
    assign_with_runtime_recovery, fixed_view_allocation, pressure_rematerialization_allocation,
};
use crate::{AllocationAdmissionError, RetainedAllocation};
use crate::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedRegisterHomeCustodyError, OptimizedSelectedReanalysisError,
};
use optimization_core::OptimizationExecutionPhase;
use selected_instruction_analyses::{
    AllocationRecoveryRule, AllocationRecoveryRuleCatalogError,
    OptimizedAllocationLegalityCustodyError, SelectedAllocationInput,
    selected_allocation_recovery_entry,
};

pub fn stage_register_allocation(
    input: SelectedAllocationInput,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let recovery = selected_allocation_recovery_entry(
        &input
            .selections()
            .project_phase(OptimizationExecutionPhase::AllocationRecovery),
    )
    .map_err(RegisterAllocationError::RecoveryCatalog)?;
    let legality = input
        .into_legality(recovery.map(|entry| entry.payload().availability()))
        .map_err(RegisterAllocationError::Legality)?;
    let staged = match recovery.map(|entry| entry.payload().rule()) {
        None => assign_with_runtime_recovery(legality)?,
        Some(AllocationRecoveryRule::FixedViewCopies(policy)) => {
            fixed_view_allocation(legality, policy)?
        }
        Some(AllocationRecoveryRule::PressureRematerialization {
            choice,
            classification,
            rematerialization,
        }) => pressure_rematerialization_allocation(
            legality,
            choice,
            classification,
            rematerialization,
        )?,
    };
    RetainedAllocation::admit(staged, recovery.map(|entry| entry.optimization()))
        .map_err(RegisterAllocationError::Admission)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocationError {
    RecoveryCatalog(AllocationRecoveryRuleCatalogError),
    FixedSegments(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    FixedViewHomes(crate::OptimizedPostCopyRegisterHomeCustodyError),
    Reanalysis(OptimizedSelectedReanalysisError),
    Rematerialization(crate::OptimizedActiveResidentRematerializationError),
    RuntimeSpill(crate::RuntimeSpillAllocationError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Homes(OptimizedRegisterHomeCustodyError),
    Admission(AllocationAdmissionError),
}

stage_error_display!(RegisterAllocationError, "register allocation failed");
