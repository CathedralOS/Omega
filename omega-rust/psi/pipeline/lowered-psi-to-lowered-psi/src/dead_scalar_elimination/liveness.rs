//! Backward scalar demand through current operation results and control edges.

use semantic_vocabulary::ValueId;
use std::collections::BTreeSet;
use terminal_psi::{OperationKind as O, TerminalMachine, Terminator};

pub(super) fn eliminate(
    machine: &mut TerminalMachine,
    source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
    retained_values: &[ValueId],
) {
    let operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let mut pending = retained_values.to_vec();
    for operation in &operations {
        if !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
            && !inputs(&operation.kind, &mut pending)
        {
            // Dynamic catalogs and guarded call provenance carry additional
            // scalar references outside the local operand list.
            return;
        }
    }
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump { arguments, .. } => pending.extend(arguments),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                pending.push(*condition);
                pending.extend(&when_true.arguments);
                pending.extend(&when_false.arguments);
            }
            Terminator::Return { value, .. } => pending.push(*value),
            Terminator::Crash { .. } => return, // Retain exact guard-term inputs.
            Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::StructuralCase { .. } => {}
        }
    }
    for occurrence in source_calls {
        if operations
            .iter()
            .any(|operation| operation.id == occurrence.terminal_operation)
        {
            pending.extend(
                occurrence
                    .source_values_before_call
                    .iter()
                    .map(|value| value.id),
            );
        }
    }
    let mut live = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !live.insert(value) {
            continue;
        }
        if let Some(producer) = operations.iter().find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == value)
        }) && !inputs(&producer.kind, &mut pending)
        {
            return;
        }
    }
    for block in &mut machine.blocks {
        block.operations.retain(|operation| {
            !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
                || operation
                    .result
                    .scalar()
                    .is_none_or(|result| live.contains(&result.id))
        });
    }
}

/// False means the operand inventory is indirect and the machine is retained.
/// The exhaustive match forces new operation variants to declare that fact.
fn inputs(operation: &O, values: &mut Vec<ValueId>) -> bool {
    match operation {
        O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::IntegerStructuralField { .. }
        | O::BooleanStructuralField { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. }
        | O::PortWrite { .. } => {}
        O::BooleanNot { operand }
        | O::IntegerBitwiseNot { operand }
        | O::IntegerWiden { operand }
        | O::IntegerExactCast { operand, .. } => values.push(*operand),
        O::BooleanEqual { left, right }
        | O::IntegerEqual { left, right }
        | O::IntegerLessThan { left, right }
        | O::IntegerLessOrEqual { left, right }
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
        O::WriteOnlyPrimitiveStore { value, .. } | O::StructuralScalarFieldStore { value, .. } => {
            values.push(*value)
        }
        O::BoundaryCall { arguments, .. } => values.extend(arguments),
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
        O::CallStructural { .. }
        | O::CallDynamicScalar { .. }
        | O::CallDynamicUnit { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallDynamicParameterUnit { .. }
        | O::StoreDynamicDescriptor { .. } => return false,
    }
    true
}
