//! Scalar `ValueId` operand rewrite dispatch shared by candidate replays.
//!
//! Every family that rewrites an operation's scalar uses walks the same
//! operand surface — establishment storages and elements, call selectors and
//! argument rosters, unary/pair/shift/fused arithmetic operands, atomic event
//! inputs, and the returned value — against the same no-operand roster. The
//! block-flow variants stay caller-owned: `Jump`, `Conditional`, and
//! `StructuralCase` carry successor bindings whose accounting differs per
//! family (a substitution rewrites them, a block-parameter rewrite retires
//! the bound position, an observation validation re-derives them), so each
//! caller matches those first and delegates every other variant here.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::ValueId;

/// Rewrite `operation`'s scalar `ValueId` operands with `replace`.
///
/// `Jump`, `Conditional`, and `StructuralCase` are no-ops in this dispatch —
/// their successor bindings are the caller's accounting.
pub(crate) fn rewrite_value_operands(operation: &mut O, mut replace: impl FnMut(&mut ValueId)) {
    match operation {
        O::RegisterAccess { event, .. } => {
            event.map_operands(|mut value| {
                replace(&mut value);
                value
            });
        }
        O::EstablishStructuralArray { .. } | O::EstablishPlacedReference { .. } => {}
        O::EstablishReference { storage, .. } => {
            if let Some(view) = storage { replace(&mut view.byte_offset); }
        }
        O::EstablishScalarArray { elements, .. } => {
            for element in elements {
                replace(element);
            }
        }
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
            for initializer in fields {
                if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut initializer.value
                {
                    replace(value);
                }
            }
        }
        // Establishment ops spell no operands — nothing to rewrite.
        O::EstablishZeroedStructural { .. } | O::EstablishStructuralLocal { .. } => {}
        // Only the stored value is rewritten. A runtime-selected path
        // element's selector stays as spelled: its accepted certificate names
        // that exact value, so a rewrite that would retire it leaves the use
        // behind and validation rejects the candidate, as for the bounded
        // byte-view operands.
        abstract_operations::primitive_local_triple_pattern!(value, ..)
        | O::StructuralScalarFieldStore { value, .. } => replace(&mut value.value),
        O::CallStructural { callee, arguments, .. } => {
            if let abstract_operations::AbstractCallTarget::Dynamic { selector, .. } = callee {
                replace(selector);
            }
            for argument in arguments {
                replace(argument);
            }
        }
        O::Call { arguments, .. }
        | O::ExternalCompletion { arguments, .. }
        | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | abstract_operations::integer_widen_or_exact_cast_pattern!(operand, ..)
        | O::ScalarRepresentationView { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(left, right, ..)
        | O::IntegerArithmetic { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::TrappingInteger { operation, .. } => operation.map_operands(&mut |mut value| {
            replace(&mut value);
            value
        }),
        O::FloatResultPolicy { policy, .. } => policy.map_operands(&mut |mut value| {
            replace(&mut value);
            value
        }),
        O::IeeeFloatCompare { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => {
            replace(left);
            replace(right);
            replace(addend);
        }
        O::IntegerShift { value, count, .. } => {
            replace(value);
            replace(count);
        }
        // Caller-owned successor-binding accounting.
        O::Jump { .. } | O::Conditional { .. } | O::StructuralCase { .. } => {}
        O::Return { value, .. } => replace(value),
        O::AtomicEvent { event, .. } => {
            // Atomic operands are scalar uses; the observed prior and
            // results are definitions the event produces, not uses.
            event.for_each_input_mut(&mut replace);
        }
        abstract_operations::establish_byte_literal_triple_pattern!(..)
        | O::PortWrite { .. }
        | O::SelectImplementation { .. }
        | abstract_operations::constant_value_triple_pattern!(..)
        | abstract_operations::scalar_read_or_membership_pattern!(..)
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | O::ByteSequenceWrite { .. }
        | abstract_operations::byte_sequence_store_pair_pattern!( .. )
        | abstract_operations::byte_view_triple_pattern!(..)
        | abstract_operations::byte_view_length_triple_pattern!(..)
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | O::ElementViewSubslice { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | O::StructuralScalarFieldRead { .. }
        | O::ReleaseReference { .. }
        // A restoration window's fields are structural places and
        // declarations — custody identity, never scalar uses a block
        // parameter rewrite may substitute.
        | abstract_operations::move_or_store_structural_field_pattern!()
        | O::StructuralLeafCopy { .. }
        | O::ReturnUnit { .. }
        | abstract_operations::return_structural_or_crash_pattern!() => {}
    }
}
