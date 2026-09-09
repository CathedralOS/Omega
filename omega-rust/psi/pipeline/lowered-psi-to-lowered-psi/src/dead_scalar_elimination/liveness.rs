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
        // This pass never removes structural storage. A primitive read has no
        // scalar operand, but remains an observation even when its result dies.
        O::PrimitiveScalarRead { .. } => {}
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
        } => {
            values.extend([*index, *value, *length]);
        }
        O::StructuralByteSequenceFieldStore { length, .. } => values.push(*length),
        O::ByteSequenceRead { index, length, .. } => values.extend([*index, *length]),
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => values.extend([*start, *end, *length]),
        O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::IntegerStructuralField { .. }
        | O::ByteSequenceLength { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::BooleanStructuralField { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. }
        | O::PortWrite { .. } => {}
        O::EstablishScalarCase { fields, .. } => {
            values.extend(fields.iter().map(|field| field.value));
        }
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
        O::EstablishPrimitiveLocal { value }
        | O::WriteOnlyPrimitiveStore { value, .. }
        | O::StructuralScalarFieldStore { value, .. } => values.push(*value),
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ObligationId, PlaceId, StructuralFieldId};

    #[test]
    fn scalar_case_construction_demands_each_payload_operand() {
        let payloads = [ValueId::new(2).unwrap(), ValueId::new(1).unwrap()];
        let operation = O::EstablishScalarCase {
            result_case: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
            fields: payloads
                .iter()
                .enumerate()
                .map(|(position, value)| terminal_psi::ScalarCaseField {
                    field: StructuralFieldId::new(position as u64 + 1).unwrap(),
                    value: *value,
                    range_obligation: None,
                })
                .collect(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert_eq!(pending, payloads);
        assert!(!terminal_semantics::is_unconditionally_total_scalar(
            &operation
        ));
    }

    #[test]
    fn indexed_byte_store_demands_all_scalar_operands() {
        let index = ValueId::new(1).unwrap();
        let value = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let operation = O::StructuralByteSequenceFieldByteStore {
            destination: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
            index,
            value,
            length,
            obligation: ObligationId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert_eq!(pending, vec![index, value, length]);
    }

    #[test]
    fn mutable_byte_view_write_retains_its_effect_and_scalar_operands() {
        let index = ValueId::new(1).unwrap();
        let value = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let operation = O::ByteSequenceWrite {
            destination: PlaceId::new(1).unwrap(),
            index,
            value,
            length,
            obligation: ObligationId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert_eq!(pending, vec![index, value, length]);
        assert!(!terminal_semantics::is_unconditionally_total_scalar(
            &operation
        ));
    }

    #[test]
    fn byte_field_length_has_structural_source_not_scalar_operand() {
        let operation = O::StructuralByteSequenceFieldLength {
            source: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&operation, &mut pending));
        assert!(pending.is_empty());
    }

    #[test]
    fn primitive_storage_initialization_demands_its_value_and_reads_remain_observations() {
        let initializer = ValueId::new(1).unwrap();
        let establishment = O::EstablishPrimitiveLocal { value: initializer };
        let read = O::PrimitiveScalarRead {
            source: PlaceId::new(1).unwrap(),
        };
        let mut pending = Vec::new();
        assert!(inputs(&establishment, &mut pending));
        assert_eq!(pending, vec![initializer]);
        pending.clear();
        assert!(inputs(&read, &mut pending));
        assert!(pending.is_empty());
        assert!(!terminal_semantics::is_unconditionally_total_scalar(
            &establishment
        ));
        assert!(!terminal_semantics::is_unconditionally_total_scalar(&read));
    }
}
