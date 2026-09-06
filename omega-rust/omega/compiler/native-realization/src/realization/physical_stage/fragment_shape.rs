//! Transitional routing predicates over current input, not trial execution.
//! Selection and publication still independently validate every admitted body.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression, TargetFunction, TargetIntegerExpression,
    TargetOperation,
};

/// The existing two-parameter comparison / constant-return selection forms.
/// Source common-return blocks, edge arguments, nested trees, calls and cleanup
/// deliberately remain outside this migration boundary.
pub(super) fn scalar_conditional(function: &AbstractFunction, native: &TargetFunction) -> bool {
    let AbstractFunctionResult::Scalar(result) = function.result else {
        return false;
    };
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let [left, right] = function.parameters.as_slice() else {
        return false;
    };
    if function.attachment.is_some()
        || !function.structural_parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || result.scalar_type != unsigned
        || left.scalar_type != right.scalar_type
        || ![unsigned, signed].contains(&left.scalar_type)
    {
        return false;
    }
    let [entry, when_true, when_false] = function.block_entries.as_slice() else {
        return false;
    };
    if entry.block != function.entry
        || [
            entry.operation_offset,
            when_true.operation_offset,
            when_false.operation_offset,
        ] != [0, 2, 4]
        || function
            .block_entries
            .iter()
            .any(|block| !block.parameters.is_empty())
    {
        return false;
    }
    let [
        comparison,
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        },
        AbstractOperation::IntegerConstant { .. },
        AbstractOperation::Return {
            cleanup_actions: true_cleanup,
            ..
        },
        AbstractOperation::IntegerConstant { .. },
        AbstractOperation::Return {
            cleanup_actions: false_cleanup,
            ..
        },
    ] = function.operations.as_slice()
    else {
        return false;
    };
    if !when_true.bindings.is_empty()
        || when_true.target != function.block_entries[1].block
        || when_false.target != function.block_entries[2].block
        || !when_false.bindings.is_empty()
        || !when_true.trivial_affine_discards.is_empty()
        || !when_false.trivial_affine_discards.is_empty()
        || !true_cleanup.is_empty()
        || !false_cleanup.is_empty()
        || !matches!(
            comparison,
            AbstractOperation::IntegerEqual { .. }
                | AbstractOperation::IntegerLessThan { .. }
                | AbstractOperation::IntegerLessOrEqual { .. }
        )
    {
        return false;
    }
    let TargetOperation::ReturnIntegerExpressionConditionalControl { condition, .. } =
        &native.operation
    else {
        return false;
    };
    let (left, right) = match condition {
        TargetBooleanExpression::IntegerEqual { left, right, .. }
            if function.parameters[0].scalar_type == unsigned =>
        {
            (left, right)
        }
        TargetBooleanExpression::IntegerLessThan { left, right, .. }
        | TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => (left, right),
        _ => return false,
    };
    matches!((left.as_ref(), right.as_ref()),
        (TargetIntegerExpression::Parameter { parameter_index: left, location: ScalarParameterLocation::Register(_), .. },
         TargetIntegerExpression::Parameter { parameter_index: right, location: ScalarParameterLocation::Register(_), .. })
        if left != right)
}
