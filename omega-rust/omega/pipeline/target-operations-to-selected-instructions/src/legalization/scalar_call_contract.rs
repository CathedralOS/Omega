//! Shared input predicates for the existing memory-free scalar call ABI.
//! These checks choose no legalization or selected output. The ordinary
//! function roster separately produces and independently replays each callee.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target_operations::TargetFunction;

pub(super) fn accepts(
    native: target::NativeTarget,
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    call: &CallPlan,
) -> bool {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let scalar = ScalarType::Integer(integer);
    let Ok(expected) = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: Some(ValueShape::integer(8, 8)),
        },
    ) else {
        return false;
    };
    let Some(abi) = &target.fixed_integer_scalar_abi else {
        return false;
    };
    target.machine == abstracted.machine
        && target.machine == optimized.machine
        && target.attachment.is_none()
        && abstracted.attachment.is_none()
        && optimized.attachment.is_none()
        && target.mixed_structural_scalar_abi.is_none()
        && *call == expected
        && abi.call_plan == expected
        && abi.parameters.len() == 2
        && abi
            .parameters
            .iter()
            .zip(&expected.parameters)
            .all(|(parameter, placement)| {
                parameter.scalar_type == integer && parameter.placement == *placement
            })
        && abi.result.scalar_type == integer
        && expected.result.as_ref() == Some(&abi.result.placement)
        && abstracted.parameters.len() == 2
        && abstracted
            .parameters
            .iter()
            .all(|parameter| parameter.scalar_type == scalar)
        && optimized.parameters.len() == 2
        && optimized
            .parameters
            .iter()
            .zip(&abstracted.parameters)
            .all(|(optimized, declared)| {
                optimized.value == declared.value && optimized.scalar_type == declared.scalar_type
            })
        && abi
            .parameters
            .iter()
            .zip(&abstracted.parameters)
            .all(|(parameter, declared)| parameter.value == declared.value)
        && matches!(abstracted.result, AbstractFunctionResult::Scalar(result)
            if result.scalar_type == scalar)
        && optimized.result == abstracted.result
        && abstracted.structural_parameters.is_empty()
        && optimized.structural_parameters.is_empty()
        && optimized.structural_places.is_empty()
        && abstracted.entry_claims.is_empty()
        && optimized.entry_claim_declarations.is_empty()
        && optimized.content_entry_claims.is_empty()
        && optimized.entry_claims.is_empty()
        && optimized.declared_places.is_empty()
        && abstracted.published_service_ceiling.is_empty()
        && optimized.published_service_ceiling.is_empty()
        && abstracted.operations.iter().all(memory_free_operation)
}

fn memory_free_operation(operation: &AbstractOperation) -> bool {
    match operation {
        AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::ExactIntegerSubtract { .. } => true,
        AbstractOperation::Return {
            cleanup_actions, ..
        } => cleanup_actions.is_empty(),
        AbstractOperation::Jump {
            trivial_affine_discards,
            ..
        } => trivial_affine_discards.is_empty(),
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => {
            when_true.trivial_affine_discards.is_empty()
                && when_false.trivial_affine_discards.is_empty()
        }
        _ => false,
    }
}
