//! Input row and control contracts. Whole-unit validation owns SSA and effect-chain validity.
use super::*;
use optimization_unit::OptimizationBlock;
use semantic_vocabulary::OperationId;
pub(in crate::legalization) fn instruction(
    node: &OptimizationNode,
) -> Option<(OperationId, Option<ValueId>)> {
    if let AbstractOperation::ByteSequenceSubslice { psi_operation, .. }
    | AbstractOperation::EstablishRecord { psi_operation, .. }
    | AbstractOperation::EstablishScalarArray { psi_operation, .. }
    | AbstractOperation::EstablishScalarCase { psi_operation, .. }
    | AbstractOperation::CallStructural { psi_operation, .. }
    | AbstractOperation::EstablishPrimitiveLocal { psi_operation, .. }
    | AbstractOperation::PrimitiveLocalStore { psi_operation, .. }
    | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
    | AbstractOperation::CallUnit { psi_operation, .. }
    | AbstractOperation::ByteSequenceWrite { psi_operation, .. }
    | AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }
    | AbstractOperation::StructuralScalarFieldStore { psi_operation, .. } = &node.operation
    {
        Some((*psi_operation, None))
    } else if let AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Structural(_),
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
        ..
    } = &node.operation
    {
        (arguments.is_empty()
            && structural_arguments.is_empty()
            && completion_claim_sources.is_empty()
            && completion_receipts.is_empty())
        .then_some((*psi_operation, None))
    } else if let AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
        ..
    } = &node.operation
    {
        (arguments.len() == 1
            && structural_arguments.is_empty()
            && completion_claim_sources.is_empty()
            && completion_receipts.is_empty())
        .then_some((*psi_operation, None))
    } else {
        scalar_instruction(node).map(|(operation, result)| (operation, Some(result)))
    }
}
fn scalar_instruction(node: &OptimizationNode) -> Option<(OperationId, ValueId)> {
    match &node.operation {
        AbstractOperation::IeeeFloatCompare {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralCaseMembership {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, result.value)),
        AbstractOperation::IeeeFloatConstant {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        AbstractOperation::CallStructuralScalar {
            psi_operation,
            result,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } if scalar_shape(result.scalar_type).is_some()
            && claim_transfers.is_empty()
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty() =>
        {
            Some((*psi_operation, result.value))
        }
        AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u8_type()) => {
            Some((*psi_operation, result.value))
        }
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u64_type()) => {
            Some((*psi_operation, result.value))
        }
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } if valid_literal(*scalar_type, *value) => Some((*psi_operation, *result)),
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            requirement_obligations,
            crash_continuations,
            ..
        } if scalar_shape(*scalar_type).is_some()
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty() =>
        {
            Some((*psi_operation, *result))
        }
        AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            ..
        } if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
            Some((*psi_operation, *result))
        }
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        } if *scalar_type == u64_type()
            || *scalar_type == u8_type()
            || *scalar_type == u32_type() =>
        {
            Some((*psi_operation, *result))
        }
        AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        AbstractOperation::BooleanEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BooleanNot {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        AbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            ..
        } => Some((*psi_operation, *result)),
        _ => None,
    }
}
fn valid_literal(scalar: ScalarType, value: semantic_vocabulary::IntegerValue) -> bool {
    if let ScalarType::Integer(integer) = scalar
        && matches!(integer.bits(), 8 | 16 | 32 | 64)
    {
        return match value {
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                integer.sign() == IntegerSign::Unsigned && value < (1_u128 << integer.bits())
            }
            semantic_vocabulary::IntegerValue::Signed(value) => {
                integer.sign() == IntegerSign::Signed
                    && value >= -(1_i128 << (integer.bits() - 1))
                    && value < (1_i128 << (integer.bits() - 1))
            }
        };
    }
    false
}
pub(super) fn validate(
    block: &OptimizationBlock,
    optimized: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (terminator, body) = block.nodes.split_last().ok_or(invalid.clone())?;
    for (position, parameter) in block.parameters.iter().enumerate() {
        if (integer_type(parameter.scalar_type).is_none()
            && !(matches!(
                parameter.scalar_type,
                ScalarType::Boolean | ScalarType::IeeeFloat(_)
            ))
            && !(matches!(parameter.scalar_type, ScalarType::Integer(_))
                && scalar_shape(parameter.scalar_type).is_some()))
            || parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position: position as u32,
                })
        {
            return Err(invalid);
        }
    }
    for (position, node) in body.iter().enumerate() {
        let (operation, result) = instruction(node).ok_or(invalid.clone())?;
        if !node.successors.is_empty()
            || node.provenance != [PsiProvenance::Operation(operation)]
            || node.fuel.is_empty()
            || node
                .fuel
                .iter()
                .any(|fuel| fuel.site != PsiProvenance::Operation(operation))
        {
            return Err(invalid);
        }
        if matches!(
            node.operation,
            AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
        ) {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !super::byte_views::contains_view(optimized, *source)
                || [start, end, length].iter().any(|value| {
                    value_type(optimized, **value) != Some(ScalarType::Integer(u64_type()))
                })
            {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || super::byte_views::mutable_parameter(optimized, *destination).is_none()
                || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *value) != Some(ScalarType::Integer(u8_type()))
            {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::StructuralScalarFieldStore {
            destination, value, ..
        }
        | AbstractOperation::WriteOnlyPrimitiveStore {
            destination, value, ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !optimized.structural_parameters.contains(destination)
                || !matches!(
                    destination.access,
                    terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                )
                || value_type(optimized, value.value) != Some(value.scalar_type)
                || scalar_shape(value.scalar_type).is_none()
            {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::EstablishByteSequenceLiteral { .. } = &node.operation {
            if result.is_some()
                || !node.definitions.is_empty()
                || !super::literals::roster(optimized)
            {
                return Err(invalid);
            }
            continue;
        }
        if matches!(
            node.operation,
            AbstractOperation::EstablishRecord { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishScalarCase { .. }
                | AbstractOperation::CallStructural { .. }
        ) {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::CallUnit {
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !claim_transfers.is_empty()
                || !requirement_obligations.is_empty()
                || !crash_continuations.is_empty()
            {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::BoundaryCall {
            arguments,
            result: boundary_result,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || match boundary_result {
                    abstract_operations::AbstractBoundaryResult::Structural(_) => {
                        !arguments.is_empty()
                    }
                    abstract_operations::AbstractBoundaryResult::Unit => {
                        arguments.len() != 1
                            || value_type(optimized, arguments[0])
                                != Some(ScalarType::Integer(i32_type()))
                    }
                    _ => true,
                }
            {
                return Err(invalid);
            }
            continue;
        }
        let [definition] = node.definitions.as_slice() else {
            return Err(invalid);
        };
        if Some(definition.value) != result
            || definition.site
                != (ValueDefinitionSite::Node {
                    block: block.id,
                    node: position as u32,
                })
        {
            return Err(invalid);
        }
        let expected_type = match &node.operation {
            AbstractOperation::IeeeFloatCompare {
                format,
                left,
                right,
                ..
            } => {
                if value_type(optimized, *left) != Some(ScalarType::IeeeFloat(*format))
                    || value_type(optimized, *right) != Some(ScalarType::IeeeFloat(*format))
                {
                    return Err(invalid);
                }
                ScalarType::Boolean
            }
            AbstractOperation::IeeeFloatConstant { value, .. } => {
                ScalarType::IeeeFloat(value.format())
            }
            AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. } => ScalarType::Boolean,
            AbstractOperation::CallStructuralScalar { result, .. }
            | AbstractOperation::PrimitiveScalarRead { result, .. }
            | AbstractOperation::IntegerStructuralField { result, .. }
            | AbstractOperation::StructuralCaseMembership { result, .. } => result.scalar_type,
            AbstractOperation::ByteSequenceRead {
                source,
                index,
                length,
                ..
            } => {
                if !super::byte_views::contains_view(optimized, *source)
                    || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                    || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(u8_type())
            }
            AbstractOperation::ByteSequenceLength { source, .. } => {
                if !super::byte_views::contains_view(optimized, *source) {
                    return Err(invalid);
                }
                ScalarType::Integer(u64_type())
            }
            AbstractOperation::IntegerConstant { scalar_type, .. }
            | AbstractOperation::Call { scalar_type, .. } => *scalar_type,
            AbstractOperation::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
                ..
            }
            | AbstractOperation::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
                ..
            } => {
                if value_type(optimized, *left) != Some(ScalarType::Integer(*scalar_type))
                    || value_type(optimized, *right) != Some(ScalarType::Integer(*scalar_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::ExactIntegerAdd { scalar_type, .. }
            | AbstractOperation::ExactIntegerSubtract { scalar_type, .. } => {
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::IntegerEqual { left, right, .. }
            | AbstractOperation::IntegerLessThan { left, right, .. }
            | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
                let left_type = value_type(optimized, *left);
                if (left_type.and_then(integer_type).is_none()
                    && left_type.and_then(integer_call_shape).is_none())
                    || value_type(optimized, *left) != value_type(optimized, *right)
                {
                    return Err(invalid);
                }
                ScalarType::Boolean
            }
            AbstractOperation::BooleanEqual { left, right, .. } => {
                if value_type(optimized, *left) != Some(ScalarType::Boolean)
                    || value_type(optimized, *right) != Some(ScalarType::Boolean)
                {
                    return Err(invalid);
                }
                ScalarType::Boolean
            }
            AbstractOperation::BooleanNot { operand, .. } => {
                if value_type(optimized, *operand) != Some(ScalarType::Boolean) {
                    return Err(invalid);
                }
                ScalarType::Boolean
            }
            AbstractOperation::IntegerWiden {
                operand,
                source_type,
                target_type,
                ..
            } => {
                if *source_type != u8_type()
                    || target_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                    || !matches!(target_type.bits(), 16 | 32 | 64)
                    || !source_type.can_widen_to(*target_type)
                    || value_type(optimized, *operand) != Some(ScalarType::Integer(*source_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*target_type)
            }
            AbstractOperation::IntegerExactCast {
                operand,
                source_type,
                target_type,
                ..
            } => {
                if !exact_cast_has_native_carriers(*source_type, *target_type)
                    || value_type(optimized, *operand) != Some(ScalarType::Integer(*source_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*target_type)
            }
            _ => return Err(invalid),
        };
        if definition.scalar_type != expected_type {
            return Err(invalid);
        }
    }
    super::control::validate(terminator, body, optimized)
}
