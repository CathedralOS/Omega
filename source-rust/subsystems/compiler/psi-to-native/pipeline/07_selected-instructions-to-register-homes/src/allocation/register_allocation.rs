/*
`stage_register_allocation` finds the catalog entry named by the build's
allocation-recovery selection. It gets legality under the allocator availability
that entry requires, or all allocatable environment views without a recovery
selection. The preceding analysis is reused only when its availability matches;
otherwise allocation derives legality from the unchanged live ranges. It then
runs the procedure the entry's rule names (plain assignment when nothing is
selected), each falling back to runtime spilling under remaining register
pressure, and admits the result.

The allocation manifest carries the program's `SelectedOptimizationRecord`
as evidence of the preceding rewrites. Allocation uses the resulting
instructions, analyses, constraints, recovery selection and work budgets.
See wiki/spec/build/optimizations.md#phase-and-product-boundaries.
*/

use crate::allocation::assignment::recovery::{
    assign_with_runtime_recovery, fixed_view_allocation, pressure_rematerialization_allocation,
};
use crate::allocation::{AllocationAdmissionError, RetainedAllocation};
use crate::allocation::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedRegisterHomeCustodyError, OptimizedSelectedReanalysisError,
};
use optimization_core::OptimizationExecutionPhase;
use selected_instruction_analyses::{
    AllocationRecoveryRule, AllocationRecoveryRuleCatalogError,
    OptimizedAllocationLegalityCustodyError, SelectedAllocationInput,
    selected_allocation_recovery_entry,
};

/// Allocation consumes the completed rewrite, including an identity rewrite.
///
/// ```compile_fail,E0308
/// fn bypass(bare: selected_instruction_analyses::SelectedAllocationInput) {
///     let _ = selected_instructions_to_register_homes::stage_register_allocation(bare);
/// }
/// ```
pub fn stage_register_allocation(
    optimized: selected_rewrites::SelectedInstructionOptimizationOutput,
) -> Result<
    (
        RetainedAllocation,
        Option<selected_rewrites::SelectedOptimizationReplay>,
    ),
    RegisterAllocationError,
> {
    let body = std::sync::Arc::clone(optimized.body_facts());
    let (input, replay) = if body.retains_stage_inputs() {
        optimized.retain_replay_for_validation()
    } else {
        (optimized.into_allocation_input(), None)
    };
    Ok((allocate(input, body)?, replay))
}

fn allocate(
    input: SelectedAllocationInput,
    body: std::sync::Arc<physical_program::NativeBodyFacts>,
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
    RetainedAllocation::admit(staged, recovery.map(|entry| entry.optimization()), body)
        .map_err(RegisterAllocationError::Admission)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocationError {
    RecoveryCatalog(AllocationRecoveryRuleCatalogError),
    FixedSegments(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    FixedViewHomes(crate::allocation::OptimizedPostCopyRegisterHomeCustodyError),
    Reanalysis(OptimizedSelectedReanalysisError),
    Rematerialization(crate::allocation::OptimizedActiveResidentRematerializationError),
    RuntimeSpill(crate::allocation::RuntimeSpillAllocationError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Homes(OptimizedRegisterHomeCustodyError),
    Admission(AllocationAdmissionError),
}

stage_error_display!(RegisterAllocationError, "register allocation failed");
