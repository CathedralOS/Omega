//! `project` turns one optimization node into its legalized scalar
//! instruction: short operation kinds are projected here and the long ones
//! by `value_instructions`, `storage_instructions`, `call_instructions` and
//! `scalar_instructions`.

use super::{
    AbstractOperation, AbstractOperationPlan, Error, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, LegalizedValueDefinition, PsiOptimizationUnit,
    TargetOperationPlan,
};
mod call_instructions;
mod scalar_instructions;
mod storage_instructions;

use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use semantic_vocabulary::IntegerValue;
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<LegalizedScalarInstruction, LegalizationError> {
    if let Some((operation, origin)) =
        scalar_graph_input::call_origin::installed_operation(node, optimized, native, plan)?
    {
        let mut call_node = node.clone();
        call_node.operation = operation;
        let mut instruction = project(&call_node, optimized, native, plan, unit, custody)?;
        let LegalizedScalarInstructionKind::Call(call) = &mut instruction.kind else {
            return Err(Error::SourceCustodyMismatch);
        };
        call.source = origin;
        call.validate_source(&node.ownership)
            .map_err(|_| Error::SourceCustodyMismatch)?;
        return Ok(instruction);
    }
    let (operation, result) =
        scalar_graph_input::instruction(node).ok_or(Error::SourceCustodyMismatch)?;
    let kind = match &node.operation {
        AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
            let (_, _, source, field) = scalar_graph_input::structural_fields::read(
                optimized,
                &node.operation,
                &plan.structural_types,
            )
            .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, field }
        }
        AbstractOperation::IntegerStructuralField { .. }
        | AbstractOperation::BooleanStructuralField { .. } => {
            let (_, _, source, field) = scalar_graph_input::structural_fields::read(
                optimized,
                &node.operation,
                &plan.structural_types,
            )
            .ok_or(Error::SourceCustodyMismatch)?;
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, field }
        }
        AbstractOperation::IntegerBitwiseAnd { left, right, .. } => {
            LegalizedScalarInstructionKind::BitwiseAnd {
                left: *left,
                right: *right,
            }
        }
        AbstractOperation::IntegerBitwiseOr { left, right, .. } => {
            LegalizedScalarInstructionKind::BitwiseOr {
                left: *left,
                right: *right,
            }
        }
        AbstractOperation::IntegerBitwiseXor { left, right, .. } => {
            LegalizedScalarInstructionKind::BitwiseXor {
                left: *left,
                right: *right,
            }
        }
        AbstractOperation::IntegerBitwiseNot { operand, .. } => {
            LegalizedScalarInstructionKind::BitwiseNot { operand: *operand }
        }
        AbstractOperation::IeeeFloatCompare {
            comparison,
            format,
            left,
            right,
            ..
        } => LegalizedScalarInstructionKind::IeeeFloatCompare {
            comparison: *comparison,
            format: *format,
            left: *left,
            right: *right,
        },
        AbstractOperation::CallStructural { .. } => call_instructions::project_call_structural(
            node, optimized, native, plan, unit, operation, custody,
        )?,
        AbstractOperation::EstablishReference { result, source, .. } => {
            LegalizedScalarInstructionKind::EstablishReference {
                result: result.clone(),
                source: source.clone(),
                shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape(),
            }
        }
        AbstractOperation::ReleaseReference { source, .. } => {
            LegalizedScalarInstructionKind::ReleaseReference { source: *source }
        }
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            LegalizedScalarInstructionKind::EstablishRecord {
                result: result.clone(),
                fields: fields.clone(),
                shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape(),
            }
        }
        AbstractOperation::EstablishScalarArray {
            result, elements, ..
        } => LegalizedScalarInstructionKind::EstablishScalarArray {
            result: result.clone(),
            elements: elements.clone(),
            shape: scalar_graph_input::scalar_arrays::shape(result, plan)?.2,
        },
        AbstractOperation::EstablishScalarCase {
            result,
            result_case,
            fields,
            ..
        } => LegalizedScalarInstructionKind::EstablishScalarCase {
            result: result.clone(),
            result_case: *result_case,
            fields: fields.clone(),
            layout: scalar_graph_input::aggregate_results::sum_layout(result, plan)?,
        },
        AbstractOperation::EstablishPrimitiveLocal { result, value, .. } => {
            LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                result: result.clone(),
                value: *value,
                shape: scalar_graph_input::scalar_shape(value.scalar_type)
                    .ok_or(Error::SourceCustodyMismatch)?,
            }
        }
        AbstractOperation::PrimitiveLocalStore {
            destination, value, ..
        } => LegalizedScalarInstructionKind::PrimitiveLocalStore {
            destination: *destination,
            value: *value,
        },
        AbstractOperation::StructuralCaseMembership { .. } => {
            storage_instructions::project_structural_case_membership(node, optimized, plan)?
        }
        AbstractOperation::PrimitiveScalarRead { source, path, .. } => {
            LegalizedScalarInstructionKind::PrimitiveScalarRead {
                source: *source,
                path: path.clone(),
            }
        }
        // An evaluated normalized foreign row precedes hosted settlement
        // probing: the row carries its own provider execution and is never a
        // hosted realization.
        AbstractOperation::BoundaryCall { .. }
            if scalar_graph_input::normalized_foreign::row(
                native,
                optimized.machine,
                operation,
            )?
            .is_some() =>
        {
            call_instructions::project_normalized_foreign_call(node, optimized, native, operation)?
        }
        AbstractOperation::BoundaryCall {
            boundary,
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } if matches!(
            scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
            target_operations::BoundaryRealization::HostedReadByte(_)
        ) =>
        {
            LegalizedScalarInstructionKind::HostedReadByte {
                boundary: *boundary,
                result: result.clone(),
                layout: scalar_graph_input::read_byte::layout(result, plan)?,
            }
        }
        AbstractOperation::BoundaryCall { .. } => {
            call_instructions::project_boundary_call(node, optimized, native, operation)?
        }
        AbstractOperation::WriteOnlyPrimitiveStore { .. } => {
            storage_instructions::project_write_only_primitive_store(node, unit)?
        }
        AbstractOperation::StructuralScalarFieldStore { .. } => {
            storage_instructions::project_structural_scalar_field_store(node, unit)?
        }
        AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            bytes,
            ..
        } => LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
            destination: *place,
            structural_type: structural_type.clone(),
            bytes: bytes.clone(),
        },
        AbstractOperation::ByteSequenceSubslice { .. } => {
            storage_instructions::project_byte_sequence_subslice(node, optimized, unit)?
        }
        AbstractOperation::CallUnit { .. } | AbstractOperation::CallStructuralScalar { .. } => {
            call_instructions::project_call_unit(
                node, optimized, native, plan, unit, operation, custody,
            )?
        }
        AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
            storage_instructions::project_structural_byte_sequence_field_byte_store(
                node, optimized, unit,
            )?
        }
        AbstractOperation::StructuralByteSequenceFieldStore { .. } => {
            storage_instructions::project_structural_byte_sequence_field_store(
                node, optimized, unit,
            )?
        }
        AbstractOperation::ByteSequenceWrite { .. } => {
            storage_instructions::project_byte_sequence_write(node, optimized, unit)?
        }
        AbstractOperation::ByteSequenceRead { .. } => {
            storage_instructions::project_byte_sequence_read(node, optimized, unit)?
        }
        AbstractOperation::ByteSequenceLength { source, .. } => {
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: *source,
                length_byte_offset: 8,
            }
        }
        AbstractOperation::BooleanNot { operand, .. } => {
            LegalizedScalarInstructionKind::BooleanNot { operand: *operand }
        }
        AbstractOperation::IntegerWiden {
            operand,
            source_type,
            ..
        } => LegalizedScalarInstructionKind::IntegerWiden {
            operand: *operand,
            source_type: *source_type,
        },
        AbstractOperation::IntegerExactCast { .. } => {
            scalar_instructions::project_integer_exact_cast(node, optimized, unit)?
        }
        AbstractOperation::IntegerConstant { value, .. } => {
            LegalizedScalarInstructionKind::Constant(*value)
        }
        AbstractOperation::IeeeFloatConstant { value, .. } => {
            let bits = match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => u128::from(*bits),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => u128::from(*bits),
            };
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(bits))
        }
        AbstractOperation::BooleanConstant { value, .. } => {
            LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(u128::from(*value)))
        }
        AbstractOperation::Call { .. } => {
            call_instructions::project_call(node, native, plan, unit)?
        }
        AbstractOperation::SaturatingIntegerSubtract { .. }
        | AbstractOperation::SaturatingIntegerAdd { .. } => {
            scalar_instructions::project_saturating_integer_add_or_subtract(node, optimized)?
        }
        AbstractOperation::SaturatingIntegerDivide { .. } => {
            scalar_instructions::project_saturating_integer_divide(node, optimized, unit)?
        }
        AbstractOperation::SaturatingIntegerRemainder { .. } => {
            scalar_instructions::project_saturating_integer_remainder(node, optimized, unit)?
        }
        AbstractOperation::WrappingIntegerAdd { left, right, .. } => {
            LegalizedScalarInstructionKind::WrappingAdd {
                left: *left,
                right: *right,
            }
        }
        AbstractOperation::WrappingIntegerSubtract { left, right, .. } => {
            LegalizedScalarInstructionKind::WrappingSubtract {
                left: *left,
                right: *right,
            }
        }
        AbstractOperation::WrappingIntegerMultiply { left, right, .. } => {
            LegalizedScalarInstructionKind::WrappingMultiply {
                left: *left,
                right: *right,
            }
        }
        AbstractOperation::WrappingIntegerRemainder { .. } => {
            scalar_instructions::project_wrapping_integer_remainder(node, optimized, unit)?
        }
        AbstractOperation::WrappingIntegerDivide { .. } => {
            scalar_instructions::project_wrapping_integer_divide(node, optimized, unit)?
        }
        AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::ExactIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerDivide { .. }
        | AbstractOperation::ExactIntegerRemainder { .. } => {
            scalar_instructions::project_exact_integer_add(node, optimized, unit)?
        }
        AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. } => {
            scalar_instructions::project_boolean_equal(node, optimized)?
        }
        _ => return Err(Error::SourceCustodyMismatch),
    };
    Ok(LegalizedScalarInstruction {
        operation,
        result: result.map(|value| LegalizedValueDefinition {
            value,
            scalar_type: node.definitions[0].scalar_type,
            definition_site: node.definitions[0].site,
        }),
        kind,
        fuel: node.fuel.clone(),
        effect: node.effect,
        ownership: node.ownership.clone(),
    })
}
