use crate::{StackSlotColoringError, StackSlotColoringPlan, ValidatedLogicalSpillOperations};

pub(super) fn validate_roots(
    source: &ValidatedLogicalSpillOperations,
    plan: &StackSlotColoringPlan,
) -> Result<(), StackSlotColoringError> {
    let logical = source.plan();
    let receipt = source.receipt();
    if receipt.identity() != crate::logical_spill_operation_identity(logical)
        || receipt.register_environment() != logical.register_environment
        || receipt.allocator_availability() != logical.allocator_availability
        || receipt.optimization_unit() != logical.optimization_unit
        || receipt.fuel_schedule() != logical.fuel_schedule
        || receipt.function_count() != logical.functions.len()
        || plan.logical_spill_operations != receipt.identity()
        || plan.register_environment != receipt.register_environment()
        || plan.allocator_availability != receipt.allocator_availability()
        || plan.optimization_unit != receipt.optimization_unit()
        || plan.fuel_schedule != receipt.fuel_schedule()
        || plan.functions.len() != logical.functions.len()
    {
        return Err(StackSlotColoringError::RootMismatch);
    }
    Ok(())
}
