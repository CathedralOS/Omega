//! Exhaustive scalar-use and incoming-binding rewrite mechanics.
use optimization_unit::RedundantBlockParameterRewrite;
use semantic_vocabulary::ValueId;

pub(crate) fn rewrite_block_parameter_operation(
    operation: &mut abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) {
    use abstract_operations::AbstractOperation as O;

    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<abstract_operations::ValueBinding>| {
        for binding in bindings.iter_mut() {
            if binding.argument == patch.parameter {
                binding.argument = patch.replacement;
            }
        }
    };
    match operation {
        O::EstablishScalarArray { elements, .. } => {
            for element in elements {
                replace(element);
            }
        }
        O::EstablishCase { fields, .. }
        | O::EstablishRecord { fields, .. } => {
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
        O::EstablishPrimitiveLocal { value, .. }
        | O::PrimitiveLocalStore { value, .. }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => replace(&mut value.value),
        O::Call { arguments, .. }
        | O::CallStructural { arguments, .. }
        | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. }
        | O::ScalarRepresentationView { operand, .. } => replace(operand),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
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
        O::Jump {
            target, bindings, ..
        } => {
            rewrite_bindings(bindings);
            if *target == patch.block {
                bindings.remove(usize::try_from(patch.position).expect("u32 fits usize"));
            }
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            replace(condition);
            for successor in [when_true, when_false] {
                rewrite_bindings(&mut successor.bindings);
                if successor.target == patch.block {
                    successor
                        .bindings
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
        O::StructuralCase { cases, .. } => {
            for successor in cases {
                if successor.target == patch.block {
                    successor
                        .payloads
                        .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                }
            }
        }
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
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallDynamic { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequencePredicate { .. }
        | O::ByteSequenceWrite { .. }
        | O::ByteSequenceStore { .. } | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceLength { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralByteSequenceFieldRead { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        // A restoration window's fields are structural places and
        // declarations — custody identity, never scalar uses a block
        // parameter rewrite may substitute.
        | O::MoveStructuralField { .. }
        | O::StoreStructuralField { .. }
        | O::StructuralLeafCopy { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}
