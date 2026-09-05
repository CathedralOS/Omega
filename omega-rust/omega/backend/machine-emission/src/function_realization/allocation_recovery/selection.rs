use super::AllocationRecoveryFunctionRelativeRealizationError;
use optimization_core::OptimizationExecutionPhase;
use selected_instructions_to_register_homes::{AllocationEvidence, AllocationOutput};

pub(super) fn validate_phase_selection(
    current: &AllocationOutput<'_>,
) -> Result<(), AllocationRecoveryFunctionRelativeRealizationError> {
    // Allocation replay already joins the exact recovery selection to its evidence.
    // This plain realization must not claim execution of any later selected phase.
    if !matches!(
        current.evidence(),
        AllocationEvidence::FixedViewCopies(_)
            | AllocationEvidence::ActiveResidentRematerialization(_)
    ) || [
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::PostAllocationMachine,
        OptimizationExecutionPhase::FunctionRelativeLayout,
    ]
    .into_iter()
    .any(|phase| !current.selections().for_phase(phase).is_empty())
    {
        return Err(AllocationRecoveryFunctionRelativeRealizationError::UnsupportedSelections);
    }
    Ok(())
}
