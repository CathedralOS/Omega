use super::super::{AbstractOperation, ScalarType, ValueId};
use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::binding_arguments;

pub fn operation_definition(operation: &AbstractOperation) -> Option<(ValueId, ScalarType)> {
    use AbstractOperation as O;
    match operation {
        O::RegisterAccess { result, .. } => result.map(|result| (result.value, result.scalar_type)),
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
        | O::SelectImplementation { result, .. }
        | abstract_operations::scalar_read_or_membership_pattern!(result, ..)
        | abstract_operations::byte_read_predicate_equal_pattern!(result, ..)
        | O::ByteSequenceLength { result, .. }
        | O::ElementViewLength { result, .. }
        | O::ElementViewRead { result, .. }
        | abstract_operations::byte_len_or_field_read_pattern!(result, ..)
        | O::StructuralScalarFieldRead { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Scalar(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(result, ..)
        | O::IeeeFloatCompare { result, .. }
        | abstract_operations::integer_equal_less_pair_pattern!(result, ..)
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        abstract_operations::bitwise_not_and_pair_pattern!(result, scalar_type, ..)
        | abstract_operations::bitwise_or_xor_pair_pattern!(result, scalar_type, ..)
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
        abstract_operations::integer_widen_or_exact_cast_pattern!(result, target_type, ..) => {
            Some((*result, ScalarType::Integer(*target_type)))
        }
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
            // The instruction-observed prior is the scalar definition a
            // read-modify-write, swap, or decisive compare-exchange produces;
            // a load defines its observed result. Store and fence define no
            // value, and a single-attempt event defines a structural outcome
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
    }
}

/// Every scalar the operation reads, in operand order: its direct operands,
/// then each runtime-selected path element's selector
/// (`AbstractOperation::runtime_indices`).
pub(super) fn operation_uses(operation: &AbstractOperation) -> Vec<ValueId> {
    let mut uses = direct_uses(operation);
    uses.extend(
        operation
            .runtime_indices()
            .into_iter()
            .map(|(index, _)| index),
    );
    uses
}

fn direct_uses(operation: &AbstractOperation) -> Vec<ValueId> {
    use AbstractOperation as O;
    match operation {
        O::RegisterAccess { event, .. } => event.inputs().copied().collect(),
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields
            .iter()
            .filter_map(|initializer| match &initializer.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(*value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            })
            .collect(),
        O::EstablishStructuralArray { .. } => Vec::new(),
        O::EstablishScalarArray { elements, .. } => elements.clone(),
        O::EstablishReference { storage, .. } => {
            storage.iter().map(|view| view.byte_offset).collect()
        }
        abstract_operations::byte_read_or_field_read_pattern!(index, length, ..) => {
            vec![*index, *length]
        }
        O::ElementViewRead { index, length, .. } => vec![*index, *length],
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        O::ByteSequencePredicate { operand, .. } => vec![operand.length()],
        O::ByteSequenceEqual { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        abstract_operations::byte_sequence_store_pair_pattern!(length, ..) => vec![*length],
        abstract_operations::byte_sequence_write_pair_pattern!(index, value, length, ..) => {
            vec![*index, *value, *length]
        }
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
        O::CallStructural {
            callee, arguments, ..
        } => callee
            .selector()
            .into_iter()
            .chain(arguments.iter().copied())
            .collect(),
        O::Call { arguments, .. }
        | O::BoundaryCall { arguments, .. }
        | O::ExternalCompletion { arguments, .. } => arguments.clone(),
        abstract_operations::primitive_local_triple_pattern!(value, ..)
        | O::StructuralScalarFieldStore { value, .. } => {
            vec![value.value]
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | abstract_operations::integer_widen_or_exact_cast_pattern!(operand, ..)
        | O::ScalarRepresentationView { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(left, right, ..)
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
        O::Jump { bindings, .. } => binding_arguments(bindings).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(binding_arguments(&when_true.bindings))
            .chain(binding_arguments(&when_false.bindings))
            .collect(),
        O::Return { value, .. } => vec![*value],
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            // Operands are real scalar uses: an atomic event reads its
            // stored value, RMW operand, or comparison/replacement pair.
            // The observed prior and results are definitions, not uses.
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
    }
}
