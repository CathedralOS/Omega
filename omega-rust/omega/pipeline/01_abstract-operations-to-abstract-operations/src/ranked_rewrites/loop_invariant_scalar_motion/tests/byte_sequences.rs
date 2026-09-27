//! Loop-invariant byte reads, byte subslices and byte literals, each
//! relocating with its root, index and length or staying inside.

use crate::VerifiedPsiOptimizationSession;
use crate::ranked_rewrites::LoopInvariantNodeResult;
use crate::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{self, recompute_psi_optimization_unit_identity};
use optimization_unit_semantics::OptimizationUnitValidationError;

use super::{
    INVARIANT_LITERAL_SOURCE, find_operation_mut, lowered_session, member_literal, operation_of,
    refresh_coordinates_and_effects, take_operation,
};

/// A byte read whose operands exercise every substitution half at once:
/// `view[i]` reads through the member `view` structural parameter that
/// resolves transitively to the machine's `entries` root, through the member
/// `i` scalar parameter that resolves transitively to the `index` machine
/// parameter, and its `length` operand is the member-internal
/// `ByteSequenceLength` (`view.len`) that must relocate in the same run —
/// its own root rebinds to `entries` so the moved read still validates
/// byte-exact against its preserved bounds obligation. The read lives in a
/// member block that dominates the component's only exit, so the
/// non-speculative gate admits it.
const MEMBER_BYTE_READ_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition i < view.len && view[i] == 0 {
                true -> tail(s, view, i, pending)
                _ -> hold(s, view, i, pending)
            }
        }
        state tail(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, view, i, pending - 1)
                _ -> finish()
            }
        }
        state hold(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition { _ -> step(s, view, i, pending) }
        }
        state finish() {}
    }
"#;

/// Same byte-read shape, but `tail`'s back edge binds `view` to a different
/// root than the entry edge: `entries` on entry and `fallback` on the back
/// edge disagree, so the member `view` parameter resolves to no single
/// representative and stays loop-carried — the read stays inside even though
/// the component's place-custody preservation is intact.
const CARRIED_VIEW_BYTE_READ_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], fallback: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining, fallback) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5], spare: &[u8]) {
            transition i < view.len && view[i] == 0 {
                true -> tail(s, view, i, pending, spare)
                _ -> hold(s, view, i, pending, spare)
            }
        }
        state tail(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5], spare: &[u8]) {
            transition pending > 0 {
                true -> step(s, spare, i, pending - 1, spare)
                _ -> finish()
            }
        }
        state hold(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5], spare: &[u8]) {
            transition { _ -> step(s, view, i, pending, spare) }
        }
        state finish() {}
    }
"#;

/// The `ByteSequenceRead` observations inside a component's member blocks,
/// as `(member block, node)` pairs in member order.
fn member_byte_reads<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| matches!(node.operation, AbstractOperation::ByteSequenceRead { .. }))
                .map(move |node| (block, node))
        })
        .collect()
}

#[test]
fn carried_view_keeps_the_byte_read_inside() {
    let session = lowered_session(CARRIED_VIEW_BYTE_READ_SOURCE, "carried view read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    assert!(
        crate::validation::place_observations::component_preserves_place_observations(
            function, component
        ),
        "no member produces or mutates a place — the refusal is the root's alone"
    );
    let reads = member_byte_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member byte read")
    };
    let (read_source, _, read_length) = match &read.operation {
        AbstractOperation::ByteSequenceRead {
            source,
            index,
            length,
            ..
        } => (*source, *index, *length),
        _ => unreachable!("member_byte_reads only yields byte reads"),
    };
    assert!(
        !crate::validation::place_observations::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .contains_key(&read_source),
        "a member-produced back-edge binding keeps the member view parameter loop-carried"
    );
    let relocating = std::collections::BTreeSet::from([read_length]);
    assert!(
        crate::validation::place_observations::invariant_byte_read_admission(
            function,
            component,
            read,
            &relocating,
            &std::collections::BTreeSet::new(),
        )
        .is_none(),
        "the shared admission refuses the carried view root"
    );
    let read_operation = operation_of(read);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != read_operation),
        "the carried-view read is not a planned relocation"
    );
}

#[test]
fn moved_byte_read_without_its_length_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (read_block, read) = member_byte_reads(function, component)[0];
    let member = read_block.id;
    let read_operation = operation_of(read);
    let (input, mut unit) = session.into_parts();
    // Hand-move the read while its `ByteSequenceLength` producer stays inside:
    // the read's `length` operand names a member-internal result no run
    // relocates, so the seed-derived admission refuses before any root or
    // operand comparison — the coupling is replayed, not trusted.
    let moved = take_operation(&mut unit, read_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_byte_read_index_rewrite_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, read) = member_byte_reads(function, component)[0];
    let read_operation = operation_of(read);
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the byte read carries an operand rewrite");
    let member = relocation.node().location().block;
    let (parameter, _) = relocation
        .node()
        .operand_rewrites()
        .first()
        .copied()
        .expect("the index substitution exists");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound index back to the member scalar parameter must fail
    // the seed-derived substitution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, read_operation);
    if let AbstractOperation::ByteSequenceRead { index, .. } = &mut forged.operation {
        *index = parameter;
    }
    forged.uses[0].value = parameter;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

/// A `StructuralByteSequenceFieldRead` spelled through the machine's `self`
/// receiver with a literal index: `self` is already the preheader-visible
/// function place and the `0` index is a member `IntegerConstant` result the
/// same relocation run preserves, so every operand stays byte-exact — the
/// accepted bounds fact records `index < length` over the operation's own
/// operand identities, and the move keeps them. The `0 < self.text.len`
/// conjunct discharges the read's bounds obligation at its source site; the
/// moved node keeps that obligation byte-exact.
const SELF_LITERAL_FIELD_BYTE_READ_SOURCE: &str = r#"
    domain [u8]::Utf8 requires valid_utf8(self);
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Root { text: [u8; 4] in Utf8; }

    machine Root::scan(&mut self, remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(&mut self, pending: u32 [0..=5]) {
            transition 0 < self.text.len && self.text[0] == 0 {
                true -> tail(pending)
                _ -> hold(pending)
            }
        }
        state tail(&mut self, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(pending - 1)
                _ -> finish()
            }
        }
        state hold(&mut self, pending: u32 [0..=5]) {
            transition { _ -> step(pending) }
        }
        state finish(&mut self) {}
    }
"#;

/// The same receiver-spelled field byte read, but its `index` is a member
/// computation result — `at + 0` — rather than a member parameter or
/// literal: the addition relocates under ordinary operand substitution
/// (`at` rebinds to `index`) while keeping its result identity, so the read
/// keeps `index` byte-exact and its recorded bounds proposition intact.
const SELF_COMPUTED_FIELD_BYTE_READ_SOURCE: &str = r#"
    domain [u8]::Utf8 requires valid_utf8(self);
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Root { text: [u8; 4] in Utf8; }

    machine Root::scan(&mut self, index: u64 [0..=2], remaining: u32 [0..=5])
    {
        transition { _ -> step(index, remaining) }
        state step(&mut self, at: u64 [0..=2], pending: u32 [0..=5]) {
            transition at + 0 < self.text.len && self.text[at + 0] == 0 {
                true -> tail(at, pending)
                _ -> hold(at, pending)
            }
        }
        state tail(&mut self, at: u64 [0..=2], pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(at, pending - 1)
                _ -> finish()
            }
        }
        state hold(&mut self, at: u64 [0..=2], pending: u32 [0..=5]) {
            transition { _ -> step(at, pending) }
        }
        state finish(&mut self) {}
    }
"#;

/// Same literal-index field byte read, but a member block stores into
/// `self.ticks`: the whole-component custody bound refuses every place
/// observation — including the read's `StructuralByteSequenceFieldLength`
/// producer — so the read stays inside even though its index is invariant.
const MUTATED_FIELD_BYTE_READ_SOURCE: &str = r#"
    domain [u8]::Utf8 requires valid_utf8(self);
    domain [u8; 4]::Utf8 requires valid_utf8(self);
    data Root { text: [u8; 4] in Utf8; ticks: u32; }

    machine Root::scan(&mut self, remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(&mut self, pending: u32 [0..=5]) {
            transition 0 < self.text.len && self.text[0] == 0 {
                true -> tail(pending)
                _ -> hold(pending)
            }
        }
        state tail(&mut self, pending: u32 [0..=5]) {
            self.ticks = 1;
            transition pending > 0 {
                true -> step(pending - 1)
                _ -> finish()
            }
        }
        state hold(&mut self, pending: u32 [0..=5]) {
            transition { _ -> step(pending) }
        }
        state finish(&mut self) {}
    }
"#;

/// The `StructuralByteSequenceFieldRead` observations inside a component's
/// member blocks, as `(member block, node)` pairs in member order.
fn member_field_byte_reads<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::StructuralByteSequenceFieldRead { .. }
                    )
                })
                .map(move |node| (block, node))
        })
        .collect()
}

/// The `StructuralByteSequenceFieldLength` observations inside a component's
/// member blocks, as `(member block, node)` pairs in member order.
fn member_field_lengths<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::StructuralByteSequenceFieldLength { .. }
                    )
                })
                .map(move |node| (block, node))
        })
        .collect()
}

#[test]
fn computed_index_field_byte_read_relocates_byte_exact() {
    let session = lowered_session(
        SELF_COMPUTED_FIELD_BYTE_READ_SOURCE,
        "computed index field byte read loop",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let reads = member_field_byte_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member field byte read")
    };
    let (read_index, read_length) = match &read.operation {
        AbstractOperation::StructuralByteSequenceFieldRead { index, length, .. } => {
            (*index, *length)
        }
        _ => unreachable!("member_field_byte_reads only yields field byte reads"),
    };
    // The `index` operand is the member `at + 0` result — its producer
    // relocates under ordinary substitution (`at` rebinds to `index`) while
    // preserving its result identity, so the read's proposition-pinned
    // operand keeps its recorded identity without any rewrite of its own.
    let index_producer = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(move |node| (block, node)))
        .map(|(_, node)| node)
        .find(|node| {
            node.definitions
                .iter()
                .any(|definition| definition.value == read_index)
        })
        .expect("the index operand has a producer");
    let index_operation = operation_of(index_producer);
    let relocating = std::collections::BTreeSet::from([read_index, read_length]);
    assert_eq!(
        crate::validation::place_observations::invariant_field_byte_read_admission(
            function,
            component,
            read,
            &relocating,
            &std::collections::BTreeSet::new(),
        )
        .map(|(_, substitution)| substitution),
        Some(std::collections::BTreeMap::new()),
        "the shared admission keeps every operand byte-exact"
    );
    let read_operation = operation_of(read);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the field byte read is a planned relocation");
    assert!(
        relocation.node().operand_rewrites().is_empty(),
        "the proposition-pinned operands admit no substitution"
    );
    let producer_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == index_operation)
        .expect("the index producer relocates in the same run");
    assert_eq!(
        producer_relocation.node().operand_rewrites().len(),
        1,
        "the index producer rebinds its member parameter operand"
    );
    assert!(
        producer_relocation.destination().node < relocation.destination().node,
        "the run keeps the index producer ahead of the read it defines"
    );

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let destination = output_function
        .blocks
        .iter()
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::StructuralByteSequenceFieldRead { index, length, .. } => {
            assert_eq!(
                *index, read_index,
                "the relocated index keeps its recorded value identity"
            );
            assert_eq!(
                *length, read_length,
                "the length operand keeps its relocated producer's result"
            );
        }
        operation => panic!("relocated read keeps its operation: {operation:?}"),
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn member_mutation_keeps_the_field_byte_read_inside() {
    let session = lowered_session(
        MUTATED_FIELD_BYTE_READ_SOURCE,
        "mutated receiver field read loop",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    assert!(
        !crate::validation::place_observations::component_preserves_place_observations(
            function, component
        ),
        "the member store to `self.ticks` refuses the whole-component custody bound"
    );
    let reads = member_field_byte_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member field byte read")
    };
    let read_operation = operation_of(read);
    let length_reads = member_field_lengths(function, component);
    let [(_, length_read)] = length_reads.as_slice() else {
        panic!("one member field length observation feeds the read")
    };
    let length_operation = operation_of(length_read);
    assert!(
        crate::validation::place_observations::invariant_field_byte_read_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
        )
        .is_none(),
        "the shared admission refuses under the failed custody bound"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    assert!(
        candidates.iter().all(|candidate| {
            candidate.relocations().iter().all(|relocation| {
                relocation.node().psi_operation() != read_operation
                    && relocation.node().psi_operation() != length_operation
            })
        }),
        "the mutated component's field read and its length observation stay inside"
    );
}

#[test]
fn moved_field_byte_read_without_its_length_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(
        SELF_LITERAL_FIELD_BYTE_READ_SOURCE,
        "literal index field byte read loop",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, read) = member_field_byte_reads(function, component)[0];
    let read_operation = operation_of(read);
    let (input, mut unit) = session.into_parts();
    // Hand-move the read while its `StructuralByteSequenceFieldLength`
    // producer stays inside: the seed's relocation run expects the coupling
    // to relocate together, so the replayed freeze refuses the drifted
    // member — the coupling is replayed, not trusted.
    let moved = take_operation(&mut unit, read_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

/// A `ByteSequenceSubslice` whose evidence is all loop-invariant: `view` is
/// the member structural parameter every reaching edge binds to `entries`,
/// its `start` is a member scalar-constant leaf, and its `end`/`length` pair
/// is the member `ByteSequenceLength` result that relocates in the same run.
/// The subslice's fresh view crosses the `step` → `tail` member edge as
/// `window`, so relocating the producer also exercises a structural result
/// whose member consumers stay inside the loop.
const MEMBER_SUBSLICE_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5])
        {
            transition { _ -> tail(s, view, view[0..view.len], i, pending) }
        }
        state tail(s: u32 in Wrapping, view: &[u8], window: &[u8], i: u64, pending: u32 [0..=5]) {
            transition pending > 0 && window.len > 0 {
                true -> step(s, view, i, pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// The `ByteSequenceSubslice` operations inside a component's member blocks,
/// as `(member block, node)` pairs in member order.
fn member_subslices<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::ByteSequenceSubslice { .. }
                    )
                })
                .map(move |node| (block, node))
        })
        .collect()
}

#[test]
fn moved_byte_subslice_without_its_length_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_SUBSLICE_SOURCE, "member subslice loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (subslice_block, subslice) = member_subslices(function, component)[0];
    let member = subslice_block.id;
    let subslice_operation = operation_of(subslice);
    let (input, mut unit) = session.into_parts();
    // Hand-move the subslice while its `ByteSequenceLength` producer stays
    // inside: the subslice's `end`/`length` operands name member-internal
    // results no run relocates, so the seed-derived admission refuses before
    // any root or operand comparison — the coupling is replayed, not trusted.
    let moved = take_operation(&mut unit, subslice_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn invariant_byte_literal_relocates_preserving_its_declared_place() {
    let session = lowered_session(INVARIANT_LITERAL_SOURCE, "invariant literal loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (literal_block, literal) = member_literal(function, component);
    let member = literal_block.id;
    let preheader = entry.source;
    let (place, structural_type, bytes) = match &literal.operation {
        AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            bytes,
            ..
        } => (*place, structural_type.clone(), bytes.clone()),
        operation => panic!("the member node is a byte literal: {operation:?}"),
    };
    assert!(crate::validation::invariant_operations::admissible_invariant_byte_literal(literal));
    let literal_operation = operation_of(literal);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == literal_operation)
        .expect("the byte literal is a planned relocation");
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    assert!(
        relocation.node().operand_rewrites().is_empty()
            && relocation.node().root_rewrite().is_none(),
        "the literal has no scalar operands or observed root to rebind"
    );
    let LoopInvariantNodeResult::LiteralPlace(declared) = relocation.node().result() else {
        panic!("the literal relocation preserves a declared place")
    };
    assert_eq!(
        *declared, place,
        "the relocation preserves the literal's declared place identity"
    );

    // The consuming `CallUnit` relocates in the same run, right behind the
    // literal whose declared root it borrows: the root is produced by the run
    // itself, so the call moves byte-exact — no argument rewrite — and still
    // spells the same place identity.
    let call = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, AbstractOperation::CallUnit { .. }))
        .expect("the member unit call exists");
    let call_operation = operation_of(call);
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the literal's consuming call relocates in the same run");
    assert!(
        matches!(
            call_relocation.node().result(),
            LoopInvariantNodeResult::Unit
        ) && call_relocation.node().argument_rewrites().is_empty()
            && call_relocation.node().operand_rewrites().is_empty(),
        "the call relocates byte-exact behind its literal producer"
    );
    assert_eq!(
        call_relocation.destination().node,
        relocation.destination().node + 1,
        "the call lands immediately behind its producer in the run"
    );

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let destination = output_function
        .blocks
        .iter()
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::EstablishByteSequenceLiteral {
            place: moved_place,
            structural_type: moved_type,
            bytes: moved_bytes,
            ..
        } => {
            assert_eq!(
                (*moved_place, moved_type.clone(), moved_bytes.as_slice()),
                (place, structural_type, bytes.as_slice()),
                "the relocated literal keeps its declaration and payload byte-exact"
            );
        }
        operation => panic!("relocated node keeps its literal operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    assert!(
        matches!(
            &moved_call.operation,
            AbstractOperation::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| argument.place == place.id)
        ),
        "the relocated call keeps borrowing the literal's preserved place"
    );
    let member_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    assert!(
        member_block.nodes.iter().all(|node| !matches!(
            node.operation,
            AbstractOperation::EstablishByteSequenceLiteral { .. }
        )),
        "the literal exists once, at the destination"
    );
    assert!(
        member_block
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::CallUnit { .. })),
        "the call exists once, at the destination behind its producer"
    );
    assert!(
        output_function.declared_places.contains(&place.id),
        "the declared literal place survives the transform"
    );
}
