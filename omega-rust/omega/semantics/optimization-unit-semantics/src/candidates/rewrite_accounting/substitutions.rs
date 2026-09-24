use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{BlockId, EdgeId, ValueId};

pub(crate) fn rewrite_scalar_value_uses(operation: &mut O, from: ValueId, to: ValueId) {
    let replace = |value: &mut ValueId| {
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
        O::EstablishScalarArray { elements, .. } => {
            for element in elements {
                replace(element);
            }
        }
        O::EstablishScalarCase { fields, .. } => {
            for field in fields {
                replace(&mut field.value);
            }
        }
        O::EstablishRecord { fields, .. } => {
            for initializer in fields {
                if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut initializer.value
                {
                    replace(value);
                }
            }
        }
        O::EstablishPrimitiveLocal { value, .. }
        | O::PrimitiveLocalStore { value, .. }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => replace(&mut value.value),
        // The runtime selector and the stored value are both scalar uses.
        O::WriteOnlyIndexedPrimitiveStore { index, value, .. } => {
            replace(&mut index.value);
            replace(&mut value.value);
        }
        O::Call { arguments, .. }
        | O::CallStructural { arguments, .. }
        | O::CallStructuralScalar { arguments, .. }
        | O::CallUnit { arguments, .. }
        | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. } => {
            replace(left);
            replace(right);
        }
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
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
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
            use abstract_operations::AbstractAtomicEvent as E;
            // Atomic operands are scalar uses; the observed prior and
            // results are definitions the event produces, not uses.
            match event {
                E::Store { value, .. } | E::Swap { value, .. } => replace(value),
                E::ReadModifyWrite { operand, .. } => replace(operand),
                E::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | E::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => {
                    replace(expected);
                    replace(replacement);
                }
                E::Load { .. } | E::Fence { .. } => {}
            }
        }
        O::DynamicDescriptorParameter { .. }
        | O::StoreDynamicDescriptor { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnitWithDynamicArguments { .. }
        | O::CallStructuralScalarWithDynamicArguments { .. }
        | O::CallDynamicScalar { .. }
        | O::CallStoredDynamicScalar { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallDynamicUnit { .. }
        | O::CallDynamicParameterUnit { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequenceWrite { .. }
        | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceLength { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::IndexedPrimitiveRead { .. }
        | O::ElementViewSubslice { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::IntegerStructuralField { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        // A restoration window's fields are structural places and
        // declarations — custody identity, never scalar uses a value
        // substitution may rewrite.
        | O::MoveStructuralField { .. }
        | O::StoreStructuralField { .. }
        | O::StructuralLeafCopy { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
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
