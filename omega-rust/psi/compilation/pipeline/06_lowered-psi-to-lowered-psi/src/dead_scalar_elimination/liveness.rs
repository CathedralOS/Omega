//! Backward scalar demand through current operation results and control edges.
//! Block parameters are demand sites, not uses: an edge argument is live only
//! when the parameter it feeds is live.

use crate::edge_arguments::drop_removed_positions;
use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind as O, TerminalMachine, Terminator};

pub(super) fn eliminate(
    machine: &mut TerminalMachine,
    source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
    retained_values: &[ValueId],
) {
    let operations = machine.operations().collect::<Vec<_>>();
    // Ranking evidence pins the covered components' parameter tables and
    // keeps the values it names live wherever they are declared; parameters
    // on blocks outside the covered components follow the ordinary rule.
    let coverage = crate::retained_identities::ranked_coverage::ranked_coverage(machine);
    let mut pending = retained_values.to_vec();
    pending.extend(coverage.values.iter().copied());
    for operation in &operations {
        if !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
            && !inputs(&operation.kind, &mut pending)
        {
            // Guarded call provenance carries additional
            // scalar references outside the local operand list.
            return;
        }
    }
    let mut incoming_arguments: BTreeMap<BlockId, Vec<Vec<ValueId>>> = BTreeMap::new();
    let mut structural_case_targets = BTreeSet::new();
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                incoming_arguments
                    .entry(*target)
                    .or_default()
                    .push(arguments.clone());
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                pending.push(*condition);
                for edge in [when_true, when_false] {
                    incoming_arguments
                        .entry(edge.target)
                        .or_default()
                        .push(edge.arguments.clone());
                }
            }
            Terminator::Return { value, .. } => pending.push(*value),
            Terminator::ExternalCompletion { arguments, .. } => {
                pending.extend(arguments.iter().copied());
            }
            Terminator::Crash { .. } => return, // Retain exact guard-term inputs.
            Terminator::StructuralCase { cases, .. } => {
                structural_case_targets.extend(cases.iter().map(|case| case.target));
            }
            Terminator::ReturnUnit { .. } | Terminator::ReturnStructural { .. } => {}
        }
    }
    // Block parameters survive removal only outside covered component member
    // blocks, outside `StructuralCase` payload targets, and with at least
    // one inventoried incoming edge. Entry blocks declare no parameters and
    // crash continuations declare only empty parameter tables, so neither
    // needs special handling.
    let mut parameter_owner: BTreeMap<ValueId, (BlockId, usize)> = BTreeMap::new();
    let mut parameter_eligible = BTreeSet::new();
    for block in &machine.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            parameter_owner.insert(parameter.id, (block.id, position));
        }
        if !coverage.blocks.contains(&block.id)
            && !structural_case_targets.contains(&block.id)
            && incoming_arguments.contains_key(&block.id)
        {
            parameter_eligible.insert(block.id);
        } else {
            pending.extend(block.parameters.iter().map(|parameter| parameter.id));
        }
    }
    for occurrence in source_calls {
        if crate::retained_identities::contains_source_call(machine, occurrence) {
            occurrence
                .frontier_before_call
                .visit_value_ids(|value| pending.push(value));
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
        }) {
            if !inputs(&producer.kind, &mut pending) {
                return;
            }
        } else if let Some(&(block, position)) = parameter_owner.get(&value)
            && let Some(edges) = incoming_arguments.get(&block)
        {
            for arguments in edges {
                pending.push(arguments[position]);
            }
        }
    }
    let mut removed_positions: BTreeMap<BlockId, Vec<usize>> = BTreeMap::new();
    for block in &mut machine.blocks {
        block.operations.retain(|operation| {
            !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
                || operation
                    .result
                    .scalar()
                    .is_none_or(|result| live.contains(&result.id))
        });
        if parameter_eligible.contains(&block.id) {
            let removed = block
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(position, parameter)| {
                    (!live.contains(&parameter.id)).then_some(position)
                })
                .collect::<Vec<_>>();
            if !removed.is_empty() {
                removed_positions.insert(block.id, removed);
            }
            block
                .parameters
                .retain(|parameter| live.contains(&parameter.id));
        }
    }
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => drop_removed_positions(arguments, removed_positions.get(target)),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    drop_removed_positions(
                        &mut edge.arguments,
                        removed_positions.get(&edge.target),
                    );
                }
            }
            _ => {}
        }
    }
}

/// False means the operand inventory is indirect and the machine is retained.
/// The exhaustive match forces new operation variants to declare that fact.
fn inputs(operation: &O, values: &mut Vec<ValueId>) -> bool {
    // A runtime-selected element in any of the operation's projections reads
    // its selector exactly like an operand.
    values.extend(
        operation
            .runtime_indexes()
            .into_iter()
            .map(|(index, _)| index),
    );
    match operation {
        O::RegisterAccess { event, .. } => values.extend(event.inputs()),
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
        | O::EstablishPlacedReference { .. }
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
