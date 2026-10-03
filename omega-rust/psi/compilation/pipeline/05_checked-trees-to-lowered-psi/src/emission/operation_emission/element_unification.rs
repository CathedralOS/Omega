//! Share one hoisted element across definitionally-equal runtime-indexed reads.
//!
//! Bounded-leaf reads materialize their element with a `StructuralLeafCopy`
//! at a `RuntimeIndex`. Two reads of the same element in different states —
//! `cells[k].v` in a transition guard and again in the target state's body —
//! each mint their own copy place. The copy itself is cheap, but proof facts
//! key on the place: a bound the guard narrowed on its read (`v < 9`) cannot
//! reach the other state's re-read because the two read different places,
//! even though the element is one and the same.
//!
//! This pass unifies such copies at the module seam: when a later copy
//! materializes an element an earlier copy already holds — same source, same
//! static path prefix, and a selector whose producing operation is an
//! identical leaf read — the later copy and its consumer are rewritten onto
//! the earlier place and the redundant copy operation is dropped, together
//! with any `OperationResult` declaration that names it as producer. Fixed-index
//! reads have always narrowed this way because their path is constant; the
//! unification gives the runtime-indexed read the same constant identity.
//!
//! Sharing is gated on two conditions, both checked against the block graph:
//!
//! - Dominance: the earlier copy's block dominates the later one's, so the
//!   hoisted place exists on every execution path reaching the read.
//! - Freshness: every path from just after the earlier copy to the later
//!   read's position runs only observation operations
//!   (`is_observation_only`), so the element it copied cannot have changed.
//!   Stores, moves, view/descriptor establishment, reference custody, calls,
//!   and port/atomic access anywhere on any path end the sharing window —
//!   a later read then keeps its own copy. Re-entering the earlier copy's
//!   block ends a path cleanly because the copy re-mints the element.
//!
//! Selector equality is deliberately narrow: two `RuntimeIndex` values are
//! equal when they are produced by identical leaf operations (field reads,
//! constant materialization). Compound selectors keep independent copies —
//! never unified, never unsound.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, ValueId};
use terminal_psi::{OperationKind, TerminalMachine, Terminator};

/// Element-hoist places are minted in the reserved `[2^62, 2^63)` band; see
/// `TERMINAL_HOISTED_ELEMENT_PLACE_BASE` in `expressions.rs`.
fn is_hoisted_element_place(place: PlaceId) -> bool {
    (place.get() >> 62) == 1
}

/// Operations that only mint fresh values or copy out material — they cannot
/// change what an already-hoisted element holds. Any kind not listed here
/// can mutate storage or permissions, so a runtime-indexed element read
/// after one must hoist again rather than share an earlier copy.
fn is_observation_only(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::StructuralByteSequenceFieldLength { .. }
            | OperationKind::StructuralByteSequenceFieldRead { .. }
            | OperationKind::StructuralCaseMembership { .. }
            | OperationKind::StructuralLeafCopy { .. }
            | OperationKind::StructuralCaseLeafCopy { .. }
            | OperationKind::ByteSequenceLength { .. }
            | OperationKind::ByteSequenceRead { .. }
            | OperationKind::ElementViewLength { .. }
            | OperationKind::ElementViewRead { .. }
            | OperationKind::IntegerConstant { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::IeeeFloatConstant { .. }
            | OperationKind::IeeeFloatCompare { .. }
            | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
            | OperationKind::BooleanNot { .. }
            | OperationKind::BooleanEqual { .. }
            | OperationKind::IntegerEqual { .. }
            | OperationKind::IntegerLessThan { .. }
            | OperationKind::IntegerLessOrEqual { .. }
            | OperationKind::IntegerStructuralField { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::IntegerBitwiseNot { .. }
            | OperationKind::IntegerWiden { .. }
            | OperationKind::IntegerExactCast { .. }
            | OperationKind::IntegerBitwiseAnd { .. }
            | OperationKind::IntegerBitwiseOr { .. }
            | OperationKind::IntegerBitwiseXor { .. }
            | OperationKind::WrappingIntegerShiftLeft { .. }
            | OperationKind::WrappingIntegerShiftRight { .. }
            | OperationKind::ExactIntegerShiftLeft { .. }
            | OperationKind::ExactIntegerShiftRight { .. }
            | OperationKind::ExactIntegerAdd { .. }
            | OperationKind::ExactIntegerSubtract { .. }
            | OperationKind::ExactIntegerMultiply { .. }
            | OperationKind::ExactIntegerDivide { .. }
            | OperationKind::ExactIntegerRemainder { .. }
            | OperationKind::WrappingIntegerDivide { .. }
            | OperationKind::WrappingIntegerRemainder { .. }
            | OperationKind::SaturatingIntegerDivide { .. }
            | OperationKind::SaturatingIntegerRemainder { .. }
            | OperationKind::WrappingIntegerAdd { .. }
            | OperationKind::SaturatingIntegerAdd { .. }
            | OperationKind::WrappingIntegerSubtract { .. }
            | OperationKind::SaturatingIntegerSubtract { .. }
            | OperationKind::WrappingIntegerMultiply { .. }
            | OperationKind::SaturatingIntegerMultiply { .. }
            | OperationKind::TrappingInteger { .. }
    )
}

/// Selector values whose producing operation is a single leaf step: a
/// structural field read, a primitive read, or a constant. Such kinds carry
/// no mutable operands, so `kind == kind` is a complete identity test.
fn is_leaf_selector_kind(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::IntegerStructuralField { .. }
            | OperationKind::BooleanStructuralField { .. }
            | OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::IntegerConstant { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::IeeeFloatConstant { .. }
    )
}

fn terminator_successors(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Jump { target, .. } => vec![*target],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![when_true.target, when_false.target],
        Terminator::StructuralCase { cases, .. } => cases.iter().map(|edge| edge.target).collect(),
        _ => Vec::new(),
    }
}

fn block_dominators(machine: &TerminalMachine, successors: &[Vec<usize>]) -> Vec<BTreeSet<usize>> {
    crate::emission::control_flow::block_dominators(
        machine
            .blocks
            .iter()
            .position(|block| block.id == machine.entry),
        successors,
    )
}

/// Blocks that can reach `target` through the successor edges.
fn reachable_to(successors: &[Vec<usize>], target: usize) -> BTreeSet<usize> {
    let mut predecessors = vec![Vec::new(); successors.len()];
    for (block, outs) in successors.iter().enumerate() {
        for &successor in outs {
            predecessors[successor].push(block);
        }
    }
    let mut seen = BTreeSet::from([target]);
    let mut work = vec![target];
    while let Some(block) = work.pop() {
        for &predecessor in &predecessors[block] {
            if seen.insert(predecessor) {
                work.push(predecessor);
            }
        }
    }
    seen
}

/// Freshness gate: every path from just after the earlier copy to the later
/// copy's position must run only observation operations — a store, call, or
/// custody op on ANY path means the hoisted element may have changed.
/// Re-entering the earlier copy's block ends a path cleanly: the copy
/// re-mints the element before the read can run again.
fn all_paths_pure(
    machine: &TerminalMachine,
    successors: &[Vec<usize>],
    can_reach: &BTreeSet<usize>,
    earlier: &ElementHoist,
    later: &ElementHoist,
) -> bool {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        machine: &TerminalMachine,
        successors: &[Vec<usize>],
        can_reach: &BTreeSet<usize>,
        earlier: &ElementHoist,
        later: &ElementHoist,
        state: (usize, usize),
        memo: &mut BTreeMap<(usize, usize), bool>,
        visiting: &mut BTreeSet<(usize, usize)>,
    ) -> bool {
        let (block, from) = state;
        if block == later.block {
            // Reached the read: only the ops ahead of the copy must observe.
            return machine.blocks[block].operations[from..later.operation]
                .iter()
                .all(|operation| is_observation_only(&operation.kind));
        }
        if let Some(&result) = memo.get(&state) {
            return result;
        }
        if !visiting.insert(state) {
            // Back-edge into a live state: a cycle whose ops already
            // verified pure — iterating it changes nothing.
            return true;
        }
        let mut pure = true;
        'scan: {
            for operation in &machine.blocks[block].operations[from..] {
                if !is_observation_only(&operation.kind) {
                    pure = false;
                    break 'scan;
                }
            }
            for &successor in &successors[block] {
                // Re-entering the earlier copy's block re-mints the element;
                // blocks that cannot reach the read never threaten it.
                if successor == earlier.block || !can_reach.contains(&successor) {
                    continue;
                }
                if !walk(
                    machine,
                    successors,
                    can_reach,
                    earlier,
                    later,
                    (successor, 0),
                    memo,
                    visiting,
                ) {
                    pure = false;
                    break 'scan;
                }
            }
        }
        visiting.remove(&state);
        memo.insert(state, pure);
        pure
    }
    walk(
        machine,
        successors,
        can_reach,
        earlier,
        later,
        (earlier.block, earlier.operation + 1),
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

/// One `StructuralLeafCopy` that materializes a runtime-indexed element.
struct ElementHoist {
    block: usize,
    operation: usize,
    source: PlaceId,
    /// Path segments before the trailing `RuntimeIndex`; always fully static
    /// because emit declines deeper indexed prefixes.
    prefix: Vec<terminal_psi::StructuralPathSegment>,
    /// The `RuntimeIndex` segment's selector value.
    index: ValueId,
    place: PlaceId,
}

/// Rewrite, per machine: later equal copies onto the earliest surviving
/// hoist's place, then drop the redundant copy operations.
pub(crate) fn unify_hoisted_element_reads(module: &mut terminal_psi::TerminalModule) {
    for machine in &mut module.machines {
        // Producer kinds for scalar results, for the selector identity test.
        let mut producers: BTreeMap<ValueId, (usize, usize)> = BTreeMap::new();
        let mut hoists = Vec::new();
        for (block_index, block) in machine.blocks.iter().enumerate() {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                if let Some(value) = operation.result.scalar() {
                    producers.insert(value.id, (block_index, operation_index));
                }
                if let (OperationKind::StructuralLeafCopy { source, path }, Some(result)) =
                    (&operation.kind, operation.result.structural())
                {
                    if is_hoisted_element_place(result.place)
                        && matches!(
                            path.last(),
                            Some(terminal_psi::StructuralPathSegment::RuntimeIndex { .. })
                        )
                    {
                        let prefix = &path[..path.len() - 1];
                        if !prefix.iter().any(|segment| {
                            matches!(
                                segment,
                                terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                            )
                        }) {
                            let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } =
                                path[path.len() - 1]
                            else {
                                unreachable!("last segment matched RuntimeIndex");
                            };
                            hoists.push(ElementHoist {
                                block: block_index,
                                operation: operation_index,
                                source: *source,
                                prefix: prefix.to_vec(),
                                index,
                                place: result.place,
                            });
                        }
                    }
                }
            }
        }
        if hoists.len() < 2 {
            continue;
        }
        let indices: BTreeMap<BlockId, usize> = machine
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id, index))
            .collect();
        let successors: Vec<Vec<usize>> = machine
            .blocks
            .iter()
            .map(|block| {
                terminator_successors(&block.terminator)
                    .into_iter()
                    .filter_map(|target| indices.get(&target).copied())
                    .collect()
            })
            .collect();
        let dominators = block_dominators(machine, &successors);
        let selectors_equal = |machine: &TerminalMachine, left: ValueId, right: ValueId| {
            if left == right {
                return true;
            }
            let kinds = (producers.get(&left), producers.get(&right));
            let (Some(&(left_block, left_op)), Some(&(right_block, right_op))) = kinds else {
                return false;
            };
            let left_kind = &machine.blocks[left_block].operations[left_op].kind;
            let right_kind = &machine.blocks[right_block].operations[right_op].kind;
            is_leaf_selector_kind(left_kind) && left_kind == right_kind
        };
        // (block, operation, superseded place, unifying place)
        let mut rewrites: Vec<(usize, usize, PlaceId, PlaceId)> = Vec::new();
        'reads: for (position, later) in hoists.iter().enumerate() {
            let dominators_of_later = &dominators[later.block];
            let can_reach = reachable_to(&successors, later.block);
            let mut unifier = None;
            for earlier in &hoists[..position] {
                if earlier.source != later.source
                    || earlier.prefix != later.prefix
                    || !dominators_of_later.contains(&earlier.block)
                    || !selectors_equal(machine, earlier.index, later.index)
                    || !all_paths_pure(machine, &successors, &can_reach, earlier, later)
                    || rewrites.iter().any(|(block, operation, _, _)| {
                        *block == earlier.block && *operation == earlier.operation
                    })
                {
                    continue;
                }
                unifier = Some(earlier);
                break;
            }
            let Some(earlier) = unifier else { continue };
            // The copy's only consumer is the leaf read emitted right after
            // it; if the op after the copy is not a field read on its place,
            // keep both copies rather than guessing at the shape.
            let reads_element = machine.blocks[later.block]
                .operations
                .get(later.operation + 1)
                .is_some_and(|operation| {
                    matches!(
                        &operation.kind,
                        OperationKind::IntegerStructuralField { source, .. }
                            | OperationKind::BooleanStructuralField { source, .. }
                            if *source == later.place
                    )
                });
            if !reads_element {
                continue 'reads;
            }
            rewrites.push((later.block, later.operation, later.place, earlier.place));
        }
        // Remove later operations first so recorded indices stay valid.
        rewrites.sort_by(|left, right| right.0.cmp(&left.0).then(right.1.cmp(&left.1)));
        for (block_index, operation_index, superseded, unified) in rewrites {
            let operations = &mut machine.blocks[block_index].operations;
            if let Some(consumer) = operations.get_mut(operation_index + 1) {
                match &mut consumer.kind {
                    OperationKind::IntegerStructuralField { source, .. }
                    | OperationKind::BooleanStructuralField { source, .. }
                        if *source == superseded =>
                    {
                        *source = unified;
                    }
                    _ => {}
                }
            }
            let dropped = operations.remove(operation_index);
            // Machine assembly may already have declared the dropped copy's
            // result: the call-operand snapshot route declares every leaf
            // copy a unit emitter produced. The producer no longer exists, so
            // its declaration goes with it; any other declaration of that
            // place is left for validation to judge.
            machine.structural_places.retain(|place| {
                place.id != superseded
                    || !matches!(
                        place.kind,
                        StructuralPlaceKind::OperationResult { producer, .. }
                            if producer == dropped.id
                    )
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A guard read and a dominated body read of `cells[k].v` share one
    /// hoisted copy. State-graph assembly declares every leaf copy it emits,
    /// so the dropped copy must take its declaration with it or the module
    /// names a missing producer.
    #[test]
    fn a_unified_element_hoist_retracts_the_dropped_copy_declaration() {
        let checked = crate::front_end::checked_program(
            "boundary trait Output { machine finish(); }
             data Cell { v: i32 [0..=9]; }
             data Main<'s> { output: &'s mut Output; cells: [Cell; 4]; k: u64 [0..=3]; }
             machine Main::main(&mut self) reaches Output {
                 self.cells[2].v = 4;
                 self.k = 2;
                 transition self.cells[self.k].v < 9 { true -> bump() false -> done() }
                 state bump(&mut self) {
                     self.cells[self.k].v = self.cells[self.k].v + 1;
                     transition self.cells[2].v == 5 { true -> done() false -> done() }
                 }
                 state done(&mut self) { self.output.finish(); }
             }",
        );
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("the runtime-indexed read-modify-write lowers");
        let machine = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        operation.result.structural().is_some_and(|result| {
                            is_hoisted_element_place(result.place)
                                && matches!(
                                    operation.kind,
                                    OperationKind::StructuralLeafCopy { .. }
                                )
                        })
                    })
            })
            .expect("the entry machine hoists its runtime-indexed element");
        let hoists = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(operation.kind, OperationKind::StructuralLeafCopy { .. })
                    && operation
                        .result
                        .structural()
                        .is_some_and(|result| is_hoisted_element_place(result.place))
            })
            .count();
        assert_eq!(hoists, 1, "the body read shares the guard's copy");
        for place in &machine.structural_places {
            if let StructuralPlaceKind::OperationResult { producer, .. } = place.kind {
                assert!(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|operation| operation.id == producer),
                    "{:?} names a dropped producer {producer:?}",
                    place.id
                );
            }
        }
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("the unified module validates");
    }
}
