use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{BlockId, EdgeId, ValueId};

pub(crate) fn rewrite_scalar_value_uses(operation: &mut O, from: ValueId, to: ValueId) {
    let mut replace = |value: &mut ValueId| {
        if *value == from {
            *value = to;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<abstract_operations::ValueBinding>| {
        for binding in bindings {
            replace(&mut binding.argument);
        }
    };
    match operation {
        O::EstablishStructuralArray { .. } => {}
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
        O::Jump { bindings, .. } => rewrite_bindings(bindings),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            rewrite_bindings(&mut when_true.bindings);
            rewrite_bindings(&mut when_false.bindings);
        }
        O::StructuralCase { .. } => {}
        O::Return { value, .. } => replace(value),
        O::AtomicEvent { event, .. } => {
            // Atomic operands are scalar uses; the observed prior and
            // results are definitions the event produces, not uses.
            event.for_each_input_mut(&mut replace);
        }
        abstract_operations::establish_byte_literal_triple_pattern!(..)
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::PortWrite { .. }
        | O::SelectImplementation { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | abstract_operations::scalar_read_or_membership_pattern!(..)
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | O::ByteSequenceWrite { .. }
        | abstract_operations::byte_sequence_store_pair_pattern!( .. )
        | abstract_operations::byte_view_triple_pattern!(..)
        | O::ByteSequenceLength { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | O::ElementViewSubslice { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | O::StructuralScalarFieldRead { .. }
        | O::ReleaseReference { .. }
        // A restoration window's fields are structural places and
        // declarations — custody identity, never scalar uses a value
        // substitution may rewrite.
        | abstract_operations::move_or_store_structural_field_pattern!()
        | O::StructuralLeafCopy { .. }
        | O::ReturnUnit { .. }
        | abstract_operations::return_structural_or_crash_pattern!() => {}
    }
}

pub(crate) fn rewrite_successor_operation(
    operation: &mut O,
    edge: EdgeId,
    target: BlockId,
    bindings: &[abstract_operations::ValueBinding],
) -> bool {
    match operation {
        O::Jump {
            psi_edge,
            target: operation_target,
            bindings: operation_bindings,
            ..
        } if *psi_edge == edge => {
            *operation_target = target;
            *operation_bindings = bindings.to_vec();
            true
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            let successor = if when_true.psi_edge == edge {
                when_true
            } else if when_false.psi_edge == edge {
                when_false
            } else {
                return false;
            };
            successor.target = target;
            successor.bindings = bindings.to_vec();
            true
        }
        _ => false,
    }
}
