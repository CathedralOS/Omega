//! Replays one legalized scalar instruction against the abstract operation
//! it legalizes: `validate` checks the short pairs here and hands the long
//! ones to `scalar_instructions`, `storage_instructions` and
//! `call_instructions`.

use super::{
    AbstractOperation, AbstractOperationPlan, Error, LegalizedOperationPlan,
    LegalizedScalarArgument, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    LegalizedValueDefinition, NativeCallOrigin, PsiOptimizationUnit, TargetOperationPlan,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use semantic_vocabulary::{IntegerValue, ScalarType};
mod aggregate_results;
mod call_instructions;
mod normalized_foreign;
mod scalar_instructions;
mod storage_instructions;
#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    if let Some((operation, origin)) =
        scalar_graph_input::call_origin::installed_operation(node, optimized, native, plan)?
    {
        let LegalizedScalarInstructionKind::Call(call) = &actual.kind else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        if call.source != origin {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
        call.validate_source(&node.ownership)
            .map_err(|_| Error::NonCanonicalLegalizedPlan)?;
        let mut call_node = node.clone();
        call_node.operation = operation;
        let mut ordinary = actual.clone();
        let LegalizedScalarInstructionKind::Call(call) = &mut ordinary.kind else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        call.source = NativeCallOrigin::Authored;
        return validate(
            &ordinary,
            &call_node,
            optimized,
            native,
            plan,
            unit,
            proposed_plan,
            custody,
        );
    }
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (operation, result) = scalar_graph_input::instruction(node).ok_or(invalid.clone())?;
    if actual.operation != operation
        || actual.result
            != result.map(|value| LegalizedValueDefinition {
                value,
                scalar_type: node.definitions[0].scalar_type,
                definition_site: node.definitions[0].site,
            })
        || actual.fuel != node.fuel
        || actual.effect != node.effect
        || actual.ownership != node.ownership
    {
        return Err(invalid);
    }
    match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::WrappingAdd { left, right },
            AbstractOperation::WrappingIntegerAdd {
                left: source_left,
                right: source_right,
                scalar_type,
                ..
            },
        ) if left == source_left
            && right == source_right
            && scalar_graph_input::scalar_shape(ScalarType::Integer(*scalar_type)).is_some()
            && [left, right].iter().all(|value| {
                scalar_graph_input::value_type(optimized, **value)
                    == Some(ScalarType::Integer(*scalar_type))
            }) => {}
        // The legalized kind must name the carrier the source operation
        // declares: a kind naming another width would clamp to the wrong
        // bounds while every register-level check still passes.
        (
            LegalizedScalarInstructionKind::SaturatingSubtract {
                carrier,
                left,
                right,
            },
            AbstractOperation::SaturatingIntegerSubtract {
                left: source_left,
                right: source_right,
                scalar_type,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::SaturatingAdd {
                carrier,
                left,
                right,
            },
            AbstractOperation::SaturatingIntegerAdd {
                left: source_left,
                right: source_right,
                scalar_type,
                ..
            },
        ) if left == source_left
            && right == source_right
            && scalar_graph_input::saturating_carrier(*scalar_type) == Some(*carrier)
            && [left, right].iter().all(|value| {
                scalar_graph_input::value_type(optimized, **value)
                    == Some(ScalarType::Integer(*scalar_type))
            }) => {}
        (
            LegalizedScalarInstructionKind::SaturatingDivide {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            },
            AbstractOperation::SaturatingIntegerDivide {
                psi_operation,
                obligation: source_obligation,
                scalar_type,
                left: source_left,
                right: source_right,
                ..
            },
        ) => {
            let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *source_obligation
            });
            let fact = facts.next().ok_or(invalid.clone())?;
            if facts.next().is_some()
                || scalar_graph_input::saturating_carrier(*scalar_type) != Some(*carrier)
                || [source_left, source_right].iter().any(|value| {
                    scalar_graph_input::value_type(optimized, **value)
                        != Some(ScalarType::Integer(*scalar_type))
                })
                || left != source_left
                || right != source_right
                || obligation != source_obligation
                || *accepted_fact != fact.identity
                || !optimized.facts.iter().any(|fact| matches!(fact,
                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if referenced == source_obligation && support == psi_operation))
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. },
            AbstractOperation::IntegerStructuralField { .. }
            | AbstractOperation::BooleanStructuralField { .. },
        )
        | (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { .. },
            AbstractOperation::StructuralByteSequenceFieldLength { .. },
        ) => storage_instructions::validate_structural_scalar_field_read(
            actual, node, optimized, plan,
        )?,
        (
            LegalizedScalarInstructionKind::BitwiseAnd { left, right },
            AbstractOperation::IntegerBitwiseAnd {
                left: source_left,
                right: source_right,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::BitwiseXor { left, right },
            AbstractOperation::IntegerBitwiseXor {
                left: source_left,
                right: source_right,
                ..
            },
        ) if left == source_left && right == source_right => {}
        (
            LegalizedScalarInstructionKind::IeeeFloatCompare {
                comparison,
                format,
                left,
                right,
            },
            AbstractOperation::IeeeFloatCompare {
                comparison: expected_comparison,
                format: expected_format,
                left: expected_left,
                right: expected_right,
                ..
            },
        ) if comparison == expected_comparison
            && format == expected_format
            && left == expected_left
            && right == expected_right => {}
        (
            LegalizedScalarInstructionKind::EstablishReference {
                result,
                source,
                shape,
            },
            AbstractOperation::EstablishReference {
                result: expected,
                source: expected_source,
                ..
            },
        ) if result == expected
            && source == expected_source
            && *shape
                == scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape() => {}
        (
            LegalizedScalarInstructionKind::ReleaseReference { source },
            AbstractOperation::ReleaseReference {
                source: expected, ..
            },
        ) if source == expected => {}
        (
            _,
            AbstractOperation::CallStructural { .. }
            | AbstractOperation::EstablishRecord { .. }
            | AbstractOperation::EstablishScalarArray { .. }
            | AbstractOperation::EstablishScalarCase { .. }
            | AbstractOperation::StructuralCaseMembership { .. },
        ) => {
            aggregate_results::validate(actual, node, optimized, native, plan, unit, custody)?;
        }
        (
            LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                result,
                value,
                shape,
            },
            AbstractOperation::EstablishPrimitiveLocal {
                result: expected,
                value: expected_value,
                ..
            },
        ) if result == expected
            && value == expected_value
            && scalar_graph_input::scalar_shape(value.scalar_type) == Some(*shape) => {}
        (
            LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value },
            AbstractOperation::PrimitiveLocalStore {
                destination: expected,
                value: expected_value,
                ..
            },
        ) if destination == expected && value == expected_value => {}
        (
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, path },
            AbstractOperation::PrimitiveScalarRead {
                source: expected,
                path: expected_path,
                ..
            },
        ) if source == expected && path == expected_path => {}
        (
            LegalizedScalarInstructionKind::HostedReadByte {
                boundary,
                result,
                layout,
            },
            AbstractOperation::BoundaryCall {
                boundary: expected_boundary,
                result: abstract_operations::AbstractBoundaryResult::Structural(expected_result),
                ..
            },
        ) if actual.has_valid_hosted_read_byte_shape()
            && boundary == expected_boundary
            && result == expected_result
            && *layout == scalar_graph_input::read_byte::layout(expected_result, plan)?
            && matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedReadByte(_)
            ) => {}
        (
            LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source },
            AbstractOperation::BoundaryCall {
                boundary: expected,
                arguments,
                ..
            },
        ) if boundary == expected
            && arguments.as_slice() == [*source]
            && matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedWriteByteI32(_)
            ) => {}
        (
            LegalizedScalarInstructionKind::HostedExitProcessI32 { boundary, source },
            AbstractOperation::BoundaryCall {
                boundary: expected,
                arguments,
                ..
            },
        ) if boundary == expected
            && arguments.as_slice() == [*source]
            && matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedExitProcessI32(_)
            ) => {}
        (
            LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. },
            AbstractOperation::WriteOnlyPrimitiveStore { .. },
        ) => storage_instructions::validate_write_only_primitive_store(actual, node, unit)?,
        (
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. },
            AbstractOperation::StructuralScalarFieldStore { .. },
        ) => storage_instructions::validate_structural_scalar_field_store(actual, node, unit)?,
        (
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                bytes,
            },
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type: expected_type,
                bytes: expected_bytes,
                ..
            },
        ) => {
            if destination != place || structural_type != expected_type || bytes != expected_bytes {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::ByteSequenceSubslice { .. },
            AbstractOperation::ByteSequenceSubslice { .. },
        ) => storage_instructions::validate_byte_sequence_subslice(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::Call(_),
            AbstractOperation::CallUnit { .. } | AbstractOperation::CallStructuralScalar { .. },
        ) => call_instructions::validate_call(
            actual,
            node,
            optimized,
            native,
            plan,
            unit,
            proposed_plan,
            operation,
            custody,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore { .. },
            AbstractOperation::StructuralByteSequenceFieldByteStore { .. },
        ) => storage_instructions::validate_structural_byte_sequence_field_byte_store(
            actual, node, optimized, plan, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { .. },
            AbstractOperation::StructuralByteSequenceFieldStore { .. },
        ) => storage_instructions::validate_structural_byte_sequence_field_store(
            actual, node, optimized, plan, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequenceWrite { .. },
            AbstractOperation::ByteSequenceWrite { .. },
        ) => storage_instructions::validate_byte_sequence_write(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequenceRead { .. },
            AbstractOperation::ByteSequenceRead { .. },
        ) => storage_instructions::validate_byte_sequence_read(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source,
                length_byte_offset: 8,
            },
            AbstractOperation::ByteSequenceLength {
                source: expected, ..
            },
        ) if source == expected => {}
        (
            LegalizedScalarInstructionKind::BooleanNot { operand },
            AbstractOperation::BooleanNot {
                operand: source, ..
            },
        ) if operand == source => {}
        (
            LegalizedScalarInstructionKind::IntegerWiden {
                operand,
                source_type,
            },
            AbstractOperation::IntegerWiden {
                operand: source,
                source_type: source_integer,
                ..
            },
        ) if operand == source && source_type == source_integer => {}
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::IntegerConstant { value, .. },
        ) if actual == value => {}
        (
            LegalizedScalarInstructionKind::IntegerExactCast { .. },
            AbstractOperation::IntegerExactCast { .. },
        ) => scalar_instructions::validate_integer_exact_cast(actual, node, optimized, unit)?,
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::IeeeFloatConstant { value, .. },
        ) if *actual
            == IntegerValue::Unsigned(match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => u128::from(*bits),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => u128::from(*bits),
            }) => {}
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::BooleanConstant { value, .. },
        ) if *actual == IntegerValue::Unsigned(u128::from(*value)) => {}
        (LegalizedScalarInstructionKind::Call(_), AbstractOperation::Call { .. }) => {
            call_instructions::validate_call_call(actual, node, native, plan, unit, proposed_plan)?
        }
        (
            LegalizedScalarInstructionKind::NormalizedForeignCall(_),
            AbstractOperation::BoundaryCall { .. },
        ) => normalized_foreign::validate(actual, node, optimized, native, plan, unit, operation)?,
        (
            LegalizedScalarInstructionKind::WrappingRemainder { .. },
            AbstractOperation::WrappingIntegerRemainder { .. },
        ) => scalar_instructions::validate_wrapping_remainder(actual, node, optimized, unit)?,
        (
            LegalizedScalarInstructionKind::ExactBinary { .. },
            AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. },
        ) => scalar_instructions::validate_exact_binary(actual, node, optimized, unit)?,
        (
            LegalizedScalarInstructionKind::Compare { .. },
            AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. },
        ) => scalar_instructions::validate_compare(actual, node, optimized)?,
        _ => return Err(invalid),
    }
    Ok(())
}
