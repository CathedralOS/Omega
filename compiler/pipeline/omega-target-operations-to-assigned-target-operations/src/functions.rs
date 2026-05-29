use omega_assigned_target_operations::{
    AssignedTargetOperationFunction, assigned_operation_span_from_target,
};

pub(crate) fn assign_function(
    function: &omega_target_operations::TargetOperationFunction,
) -> AssignedTargetOperationFunction {
    AssignedTargetOperationFunction {
        symbol: std::sync::Arc::clone(&function.symbol),
        source_key: function.source_key,
        instructions: assigned_operation_span_from_target(function.instructions),
    }
}
