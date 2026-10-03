//! Independently reconstructed scalar definitions and uses.
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::{ValueDefinition, ValueDefinitionSite, ValueUse};
use semantic_vocabulary::{BlockId, ScalarType};

pub(crate) fn expected_definitions(
    operation: &abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    use abstract_operations::AbstractOperation as O;
    let definition = match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::IeeeFloatConstant { result, value, .. } => {
            Some((*result, ScalarType::IeeeFloat(value.format())))
        }
        O::NearestIeeeFloatFusedMultiplyAdd { result, format, .. }
        | O::FloatResultPolicy { result, format, .. } => {
            Some((*result, ScalarType::IeeeFloat(*format)))
        }
        O::CallStructural {
            result: AbstractStructuralCallResult::Scalar(result),
            ..
        }
        | O::CallDynamic {
            result: abstract_operations::AbstractDynamicCallResult::Scalar(result),
            ..
        }
        | O::PrimitiveScalarRead { result, .. }
        | O::StructuralCaseMembership { result, .. }
        | O::ByteSequenceRead { result, .. }
        | O::ByteSequencePredicate { result, .. }
        | O::ByteSequenceEqual { result, .. }
        | O::ByteSequenceLength { result, .. }
        | O::ElementViewLength { result, .. }
        | O::ElementViewRead { result, .. }
        | O::StructuralByteSequenceFieldLength { result, .. }
        | O::StructuralByteSequenceFieldRead { result, .. }
        | O::StructuralScalarFieldRead { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Scalar(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IeeeFloatCompare { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::IntegerArithmetic {
            result,
            scalar_type,
            ..
        }
        | O::TrappingInteger {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::IntegerShift {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        O::ScalarRepresentationView {
            result,
            target_type,
            ..
        } => Some((*result, *target_type)),
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            // A load defines its observed result; a read-modify-write,
            // swap, or decisive compare-exchange defines the
            // instruction-observed prior. Store and fence define no value,
            // and a single-attempt event defines a structural outcome
            // rather than a scalar.
            match event {
                E::Load { result, .. } => Some((result.value, result.scalar_type)),
                E::ReadModifyWrite { prior, .. }
                | E::Swap { prior, .. }
                | E::CompareExchange {
                    observed: prior, ..
                } => Some((prior.value, prior.scalar_type)),
                E::Store { .. } | E::CompareExchangeOnce { .. } | E::Fence { .. } => None,
            }
        }
        _ => None,
    };
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

pub(crate) fn expected_uses(
    operation: &abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    use abstract_operations::AbstractOperation as O;
    let values = match operation {
        O::EstablishStructuralArray { .. } => Vec::new(),
        O::EstablishScalarArray { elements, .. } => elements.clone(),
        O::EstablishCase { fields, .. } | O::EstablishRecord { fields, .. } => fields
            .iter()
            .filter_map(|initializer| match &initializer.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(*value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            })
            .collect(),
        O::ByteSequenceRead { index, length, .. }
        | O::StructuralByteSequenceFieldRead { index, length, .. } => vec![*index, *length],
        O::ElementViewRead { index, length, .. } => vec![*index, *length],
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        O::ByteSequencePredicate { operand, .. } => vec![operand.length()],
        O::ByteSequenceEqual { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        O::ByteSequenceStore { length, .. }
        | O::StructuralByteSequenceFieldStore { length, .. } => vec![*length],
        O::ByteSequenceWrite {
            index,
            value,
            length,
            ..
        }
        | O::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            ..
        } => vec![*index, *value, *length],
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => vec![*start, *end, *length],
        O::ElementViewSubslice {
            start, end, length, ..
        } => vec![*start, *end, *length],
        O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. },
            ..
        } => vec![*start, *end],
        O::Call { arguments, .. }
        | O::CallStructural { arguments, .. }
        | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::EstablishPrimitiveLocal { value, .. }
        | O::PrimitiveLocalStore { value, .. }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => {
            vec![value.value]
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. }
        | O::ScalarRepresentationView { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::IntegerArithmetic { left, right, .. } => vec![*left, *right],
        O::TrappingInteger { operation, .. } => operation.operands(),
        O::FloatResultPolicy { policy, .. } => policy.operands(),
        O::IeeeFloatCompare { left, right, .. } => vec![*left, *right],
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => vec![*left, *right, *addend],
        O::IntegerShift { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            // Operands are real scalar uses: an atomic event reads its
            // stored value, read-modify-write operand, or
            // comparison/replacement pair. The observed prior and results
            // are definitions, not uses.
            match event {
                E::Store { value, .. } | E::Swap { value, .. } => vec![*value],
                E::ReadModifyWrite { operand, .. } => vec![*operand],
                E::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | E::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => vec![*expected, *replacement],
                E::Load { .. } | E::Fence { .. } => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    // Runtime-selected path elements read their selectors after the direct
    // operands, as the unit's own use index records them.
    values
        .into_iter()
        .chain(
            operation
                .runtime_indices()
                .into_iter()
                .map(|(index, _)| index),
        )
        .map(|value| ValueUse { value, block, node })
        .collect()
}
