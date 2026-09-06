//! Transitional routing predicates over current input, not trial execution.
//! Selection and publication still independently validate every admitted body.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression, TargetFunction, TargetIntegerExpression,
    TargetOperation,
};

/// The existing two-parameter comparison / constant-return selection forms.
/// Shared-return source blocks retain their edges and arguments through the
/// same stages. Nested trees, calls and cleanup remain outside this migration.
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
    if function.block_entries.len() == 4 {
        return shared_return(function, native);
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

fn shared_return(function: &AbstractFunction, native: &TargetFunction) -> bool {
    let [entry, join, when_true, when_false] = function.block_entries.as_slice() else {
        return false;
    };
    if entry.block != function.entry
        || !entry.parameters.is_empty()
        || [
            entry.operation_offset,
            join.operation_offset,
            when_true.operation_offset,
            when_false.operation_offset,
        ] != [0, 2, 3, 5]
        || join.parameters.len() != 1
    {
        return false;
    }
    let [
        comparison,
        AbstractOperation::Conditional {
            when_true: true_edge,
            when_false: false_edge,
            ..
        },
        AbstractOperation::Return {
            value,
            cleanup_actions,
            ..
        },
        AbstractOperation::IntegerConstant { .. },
        AbstractOperation::Jump {
            target: true_target,
            trivial_affine_discards: true_discards,
            ..
        },
        AbstractOperation::IntegerConstant { .. },
        AbstractOperation::Jump {
            target: false_target,
            trivial_affine_discards: false_discards,
            ..
        },
    ] = function.operations.as_slice()
    else {
        return false;
    };
    if *value != join.parameters[0].value
        || !cleanup_actions.is_empty()
        || *true_target != join.block
        || *false_target != join.block
        || !true_discards.is_empty()
        || !false_discards.is_empty()
        || true_edge.target != when_true.block
        || false_edge.target != when_false.block
        || !true_edge.trivial_affine_discards.is_empty()
        || !false_edge.trivial_affine_discards.is_empty()
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
            if function.parameters[0].scalar_type
                == ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()) =>
        {
            (left, right)
        }
        TargetBooleanExpression::IntegerLessThan { left, right, .. }
        | TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => (left, right),
        _ => return false,
    };
    matches!((left.as_ref(),right.as_ref()),
        (TargetIntegerExpression::Parameter { parameter_index:left, location:ScalarParameterLocation::Register(_), .. },
         TargetIntegerExpression::Parameter { parameter_index:right, location:ScalarParameterLocation::Register(_), .. }) if left != right)
}
