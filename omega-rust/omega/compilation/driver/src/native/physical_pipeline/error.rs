use machine_emission::FunctionRelativeOptimizationRealizationError;
use register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use target_operations_to_selected_instructions::OptimizedSelectionPipelineError;

#[derive(Debug)]
pub(crate) enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(
        selected_instructions_to_selected_instructions::SelectedInstructionOptimizationError,
    ),
    UnconsumedPostTerminalPhase(optimization_core::OptimizationExecutionPhase),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(selected_instructions_to_register_homes::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

// Preserve the public diagnostic while the stage owns the refusal. There is
// no selection-policy check in the coordinator.
impl From<OptimizedPostAllocationMachinePipelineError> for OptimizedVerifiedPhysicalPipelineError {
    fn from(error: OptimizedPostAllocationMachinePipelineError) -> Self {
        match error {
            OptimizedPostAllocationMachinePipelineError::UnconsumedOptimizationPhase(phase) => {
                Self::UnconsumedPostTerminalPhase(phase)
            }
            error => Self::PostAllocationMachine(error),
        }
    }
}

impl std::fmt::Display for OptimizedVerifiedPhysicalPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("common physical staging failed: ")?;
        match self {
            Self::SelectedOptimization(error) => {
                write!(formatter, "SelectedOptimization({error:?})")
            }
            Self::UnconsumedPostTerminalPhase(phase) => {
                write!(formatter, "UnconsumedPostTerminalPhase({phase:?})")
            }
            Self::RegisterEnvironment(error) => write!(formatter, "RegisterEnvironment({error:?})"),
            Self::Selection(error) => write!(formatter, "Selection({error:?})"),
            Self::RegisterAllocation(error) => write!(formatter, "RegisterAllocation({error:?})"),
            Self::PostAllocationMachine(error) => {
                write!(formatter, "PostAllocationMachine({error:?})")
            }
            Self::FunctionRelativeRealization(error) => {
                write!(formatter, "FunctionRelativeRealization({error:?})")
            }
        }
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}
