//! Input row and control contracts. Whole-unit validation owns SSA and effect-chain validity.
use super::{
    AbstractOperation, IntegerSign, OptimizationNode, PsiOptimizationFunction, PsiProvenance,
    ScalarType, ValueDefinitionSite, ValueId, scalar_shape,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::exact_cast_has_native_carriers;
use crate::legalization::scalar_graph_input::integer_call_shape;
use crate::legalization::scalar_graph_input::integer_type;
use crate::legalization::scalar_graph_input::u8_type;
use crate::legalization::scalar_graph_input::u64_type;
use crate::legalization::scalar_graph_input::value_type;
use crate::legalization::scalar_graph_input::{
    saturating_carrier, supports_signed_wrapping_remainder,
};
use optimization_unit::OptimizationBlock;
use semantic_vocabulary::OperationId;
/// Why a node has no legal instruction row. Only the first kind is a custody
/// defect; the second is a limit of this stage that the diagnostic must name.
pub(in crate::legalization) enum NodeRejection {
    /// A listed family whose own payload violates the representation: an
    /// out-of-range literal, a byte read that does not produce u8, a length
    /// that is not u64.
    Malformed,
    /// A well-formed operation with no legalized kind at its scalar type, or
    /// with a payload (crash continuations, claim transfers) this stage does
    /// not realize.
    UnsupportedFamily,
}

pub(in crate::legalization) fn instruction(
    node: &OptimizationNode,
) -> Option<(OperationId, Option<ValueId>)> {
    admit(node).ok()
}

/// Admit one node to the ordinary scalar graph, distinguishing a malformed
/// node from an operation family this stage cannot yet legalize.
pub(in crate::legalization) fn admit(
    node: &OptimizationNode,
) -> Result<(OperationId, Option<ValueId>), NodeRejection> {
    if let AbstractOperation::ByteSequenceSubslice { psi_operation, .. }
    | AbstractOperation::EstablishRecord { psi_operation, .. }
    | AbstractOperation::EstablishReference { psi_operation, .. }
    | AbstractOperation::ReleaseReference { psi_operation, .. }
    | AbstractOperation::EstablishScalarArray { psi_operation, .. }
    | AbstractOperation::EstablishScalarCase { psi_operation, .. }
    | AbstractOperation::CallStructural { psi_operation, .. }
    | AbstractOperation::EstablishPrimitiveLocal { psi_operation, .. }
    | AbstractOperation::PrimitiveLocalStore { psi_operation, .. }
    | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
    | AbstractOperation::CallUnit { psi_operation, .. }
    | AbstractOperation::ByteSequenceWrite { psi_operation, .. }
    | AbstractOperation::StructuralByteSequenceFieldByteStore { psi_operation, .. }
    | AbstractOperation::StructuralByteSequenceFieldStore { psi_operation, .. }
    | AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }
    | AbstractOperation::StructuralScalarFieldStore { psi_operation, .. } = &node.operation
    {
        Ok((*psi_operation, None))
    } else if let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        ..
    } = &node.operation
    {
        // Exact native boundary realization is checked by the source reader;
        // operand origin and result kind do not change operation identity.
        Ok((*psi_operation, result.scalar().map(|result| result.value)))
    } else {
        scalar_instruction(node).map(|(operation, result)| (operation, Some(result)))
    }
}
fn scalar_instruction(node: &OptimizationNode) -> Result<(OperationId, ValueId), NodeRejection> {
    match &node.operation {
        AbstractOperation::IeeeFloatCompare {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
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
        } => Ok((*psi_operation, result.value)),
        AbstractOperation::IeeeFloatConstant {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
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
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u8_type()) => {
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u64_type()) => {
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } if valid_literal(*scalar_type, *value) => Ok((*psi_operation, *result)),
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            crash_continuations,
            ..
        } if scalar_shape(*scalar_type).is_some() && crash_continuations.is_empty() => {
            Ok((*psi_operation, *result))
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
            Ok((*psi_operation, *result))
        }
        AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        } if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
            Ok((*psi_operation, *result))
        }
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        } if supports_signed_wrapping_remainder(*scalar_type) => Ok((*psi_operation, *result)),
        AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        } if saturating_carrier(*scalar_type).is_some() => Ok((*psi_operation, *result)),
        AbstractOperation::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        } if *scalar_type == u64_type() => Ok((*psi_operation, *result)),
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
        }
        | AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        } if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
            // Exact add/sub/mul use the full-register operation for every
            // fixed native carrier. Inputs are sign/zero normalized; the
            // retained representability proof makes the result canonical
            // without the truncation needed by wrapping arithmetic.
            // Signedness is not an admission fence, and this does not widen
            // exact division.
            Ok((*psi_operation, *result))
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
        } => Ok((*psi_operation, *result)),
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
        } => Ok((*psi_operation, *result)),
        AbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        // A guard above failed for a listed family: an invalid literal or a
        // byte read/length whose result type the representation fixes is a
        // malformed plan, not a missing lowering.
        AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::ByteSequenceRead { .. }
        | AbstractOperation::ByteSequenceLength { .. }
        | AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
            Err(NodeRejection::Malformed)
        }
        _ => Err(NodeRejection::UnsupportedFamily),
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
        let (operation, result) = admit(node).map_err(|rejection| match rejection {
            NodeRejection::Malformed => invalid.clone(),
            NodeRejection::UnsupportedFamily => LegalizationError::UnsupportedScalarOperation {
                machine: optimized.machine,
                operation: node.operation.clone(),
            },
        })?;
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
        if let AbstractOperation::StructuralByteSequenceFieldStore { length, .. } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
            {
                return Err(invalid);
            }
            continue;
        }
        if let AbstractOperation::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *value) != Some(ScalarType::Integer(u8_type()))
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
        if let AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || super::literals::declaration_producer(optimized, place.id)
                    != Some((*psi_operation, structural_type.id))
            {
                return Err(invalid);
            }
            continue;
        }
        if matches!(
            node.operation,
            AbstractOperation::EstablishRecord { .. }
                | AbstractOperation::EstablishReference { .. }
                | AbstractOperation::ReleaseReference { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishScalarCase { .. }
                | AbstractOperation::CallStructural { .. }
        ) {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(invalid);
            }
            continue;
        }
        // Requirement obligations are discharged proof metadata carried on the
        // call for correspondence, matching the admitted scalar `Call` lane;
        // claim transfers and crash continuations still reject here.
        if let AbstractOperation::CallUnit {
            claim_transfers,
            crash_continuations,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !claim_transfers.is_empty()
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
            if arguments
                .iter()
                .any(|value| value_type(optimized, *value).is_none())
            {
                return Err(invalid);
            }
            if boundary_result.scalar().is_none() {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(invalid);
                }
                continue;
            }
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
            | AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Scalar(result),
                ..
            }
            | AbstractOperation::PrimitiveScalarRead { result, .. }
            | AbstractOperation::IntegerStructuralField { result, .. }
            | AbstractOperation::StructuralByteSequenceFieldLength { result, .. }
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
            AbstractOperation::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
                ..
            }
            | AbstractOperation::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
                ..
            } => {
                if saturating_carrier(*scalar_type).is_none()
                    || value_type(optimized, *left) != Some(ScalarType::Integer(*scalar_type))
                    || value_type(optimized, *right) != Some(ScalarType::Integer(*scalar_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
                ..
            } => {
                if saturating_carrier(*scalar_type).is_none()
                    || value_type(optimized, *left) != Some(ScalarType::Integer(*scalar_type))
                    || value_type(optimized, *right) != Some(ScalarType::Integer(*scalar_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
                ..
            } => {
                if scalar_shape(ScalarType::Integer(*scalar_type)).is_none()
                    || [left, right].iter().any(|value| {
                        value_type(optimized, **value) != Some(ScalarType::Integer(*scalar_type))
                    })
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
                ..
            } => {
                if !supports_signed_wrapping_remainder(*scalar_type)
                    || value_type(optimized, *left) != Some(ScalarType::Integer(*scalar_type))
                    || value_type(optimized, *right) != Some(ScalarType::Integer(*scalar_type))
                {
                    return Err(invalid);
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::ExactIntegerAdd { scalar_type, .. }
            | AbstractOperation::ExactIntegerSubtract { scalar_type, .. }
            | AbstractOperation::ExactIntegerMultiply { scalar_type, .. }
            | AbstractOperation::ExactIntegerDivide { scalar_type, .. } => {
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
