//! Complete scalar inputs of the operations scalar demand supports.

use semantic_vocabulary::ValueId;
use terminal_psi::OperationKind as O;

/// False means the operand inventory is indirect and the machine is retained.
/// The exhaustive match forces new operation variants to declare that fact.
pub(super) fn inputs(operation: &O, values: &mut Vec<ValueId>) -> bool {
    // A runtime-selected element in any of the operation's projections reads
    // its selector exactly like an operand.
    values.extend(
        operation
            .runtime_indexes()
            .into_iter()
            .map(|(index, _)| index),
    );
    match operation {
        O::ReadScalarApplication { .. }
        | O::EstablishNativeCodeLayout { .. }
        | O::ReleaseActivationClaim { .. }
        | O::MergeActivationClaim { .. } => {}
        O::EstablishActivationClaim { committed, .. }
        | O::SplitActivationClaim {
            committed_at: committed,
            ..
        } => values.push(*committed),
        O::ActivateTaskRegister { selector, .. } | O::ReloadSegmentDescriptors { selector, .. } => values.push(*selector),
        O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => {}
        O::RegisterAccess { event, .. } => values.extend(event.inputs()),
        // This pass never removes structural storage. A primitive read has no
        // scalar operand, but remains an observation even when its result dies.
        O::GlobalDescriptorTable { .. } | O::StorageAddress { .. } | O::PrimitiveScalarRead { .. }
        | O::MoveStructuralValue { .. } | O::TransformContent { .. } => {}
        O::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            ..
        }
        | O::ByteSequenceWrite {
            index,
            value,
            length,
            ..
        }
        | O::ElementViewWrite {
            index,
            value,
            length,
            ..
        } => {
            values.extend([*index, *value, *length]);
        }
        O::ByteSequenceStore { length, .. } | O::StructuralByteSequenceFieldStore { length, .. } => values.push(*length),
        O::EstablishByteSequenceConcatenation { left, right, .. }
        | O::ByteSequenceEqual { left, right } => {
            values.extend([left.length(), right.length()]);
        }
        O::ByteSequencePredicate { operand, .. } => values.push(operand.length()),
        O::ByteSequenceRead { index, length, .. }
        | O::StructuralByteSequenceFieldRead { index, length, .. } => {
            values.extend([*index, *length])
        }
        O::ByteSequenceSubslice {
            start, end, length, ..
        }
        | O::ElementViewSubslice {
            start, end, length, ..
        } => values.extend([*start, *end, *length]),
        O::ElementViewRead { index, length, .. } => values.extend([*index, *length]),
        O::IntegerConstant { .. }
        | O::SelectImplementation { .. }
        | O::BooleanConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::ScalarStructuralField { .. }
        | O::ByteSequenceLength { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::BooleanStructuralField { .. }
        | O::StructuralCaseMembership { .. }
        // Borrowed-storage window operations move custody of structural
        // places only; they carry no scalar operands.
        | terminal_psi::move_or_store_structural_field_pattern!()
        | terminal_psi::structural_leaf_copy_pair_pattern!()
        | O::EstablishByteSequenceLiteral { .. }
        | O::ElementViewLength { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishStructuralArray { .. }
        | O::ReleaseReference { .. }
        | O::EstablishAccessReference { .. }
        | O::PortWrite { .. }
        | O::AtomicFence { .. } => {}
        O::EstablishReference { storage, .. } => {
            // The carrier captures its storage offset at formation. Keeping
            // structural custody also keeps this scalar geometry alive, even
            // when no later scalar operation reads the offset directly.
            if let Some(storage) = storage {
                values.push(storage.byte_offset);
            }
        }
        terminal_psi::element_view_establish_pair_pattern!(region, ..) => {
            // Formation consumes the captured byte endpoints even when no
            // later scalar observes them outside the structural descriptor.
            if let terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. } = region {
                values.extend([*start, *end]);
            } else if let terminal_psi::ElementViewRegion::WholeRepresentation { source_length, .. } = region {
                values.push(*source_length);
            }
        }
        O::EstablishScalarCase { fields, .. } => {
            values.extend(fields.iter().map(|field| field.value));
        }
        O::EstablishRecord { fields }
        | O::EstablishStructuralCase { fields, .. } => {
            values.extend(fields.iter().filter_map(|field| match field.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            }));
        }
        O::EstablishScalarArray { elements } => values.extend(elements),
        O::BooleanNot { operand }
        | O::IntegerBitwiseNot { operand }
        | O::IntegerWiden { operand }
        | O::IntegerExactCast { operand, .. }
        | O::ScalarRepresentationView { operand, .. } => values.push(*operand),
        O::BooleanEqual { left, right }
        | O::IeeeFloatCompare { left, right, .. }
        | O::IntegerEqual { left, right }
        | terminal_psi::integer_less_or_equal_pair_pattern!(left, right, ..)
        | O::IntegerBitwiseAnd { left, right }
        | O::IntegerBitwiseOr { left, right }
        | O::IntegerBitwiseXor { left, right }
        | O::WrappingIntegerAdd { left, right }
        | O::SaturatingIntegerAdd { left, right }
        | O::WrappingIntegerSubtract { left, right }
        | O::SaturatingIntegerSubtract { left, right }
        | O::WrappingIntegerMultiply { left, right }
        | O::SaturatingIntegerMultiply { left, right }
        | O::ExactIntegerAdd { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. } => values.extend([*left, *right]),
        O::WrappingIntegerShiftLeft { value, count }
        | O::WrappingIntegerShiftRight { value, count }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => values.extend([*value, *count]),
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } => values.extend([*left, *right, *addend]),
        O::EstablishPrimitiveLocal { value, .. }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => values.push(*value),
        O::BoundaryCall { arguments, .. } => values.extend(arguments),
        // A Trapping primitive is never removed (it is not an unconditionally
        // total scalar: a trap is an effect even when the result is dead), so
        // its operands stay live through it.
        O::TrappingInteger { operation } => values.extend(operation.operands()),
        O::FloatResultPolicy { policy } => values.extend(policy.operands()),
        // An atomic event is a memory event, never an unconditionally total
        // scalar: it survives even when its observed prior dies, so its
        // operands stay live through it.
        O::AtomicAccess { event, .. } => values.extend(event.operands()),
        O::Call {
            arguments,
            crash_continuations,
            ..
        }
        | O::CallUnit {
            arguments,
            crash_continuations,
            ..
        }
        | O::CallStructuralScalar {
            arguments,
            crash_continuations,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            arguments,
            crash_continuations,
            ..
        } => {
            if !crash_continuations.is_empty() {
                return false;
            }
            values.extend(arguments);
        }
        // The selector is an operand: the dispatched table row is live data.
        O::CallDynamic {
            selector,
            arguments,
            family_actuals,
            crash_continuations,
            ..
        } => {
            if !crash_continuations.is_empty() {
                return false;
            }
            values.push(*selector);
            values.extend(arguments);
            values.extend(family_actuals);
        }
        O::CallStructural { .. } => return false,
    }
    true
}
