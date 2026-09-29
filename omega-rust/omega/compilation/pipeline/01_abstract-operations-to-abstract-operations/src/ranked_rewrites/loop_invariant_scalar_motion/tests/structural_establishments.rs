//! Loop-invariant structural establishments: records, scalar arrays,
//! scalar cases, affine records and trivial affine locals, and the disposal
//! custody a relocated establishment re-expresses.

use crate::ranked_rewrites::LoopInvariantNodeResult;
use crate::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use optimization_unit::{PsiOptimizationUnit, recompute_psi_optimization_unit_identity};
use optimization_unit_semantics::OptimizationUnitValidationError;

use super::{
    MEMBER_SCALAR_ARRAY_SOURCE, find_operation_mut, lowered_session, lowered_session_entry,
    lowered_session_entry_with_module_edit, member_record_establishments,
    member_scalar_array_establishments, member_scalar_case_establishments, operation_of,
};

/// A two-state cycle whose member state establishes a record from invariant
/// scalars, copies it through the next member block's structural parameter,
/// and nests that copy inside a second record: `Pair { a: s, b: s }` reads
/// only the `s` member parameter that resolves transitively to `scale`,
/// `pair`'s place flows `Owned` into the copy block's member structural
/// parameter, and `Holder { inner: copy }` copies the parameter's root — so
/// the `pair` establishment relocates rebinding both scalar fields to the
/// preheader anchor, and the `holder` establishment relocates behind it
/// rebinding its copied field root from the member parameter to `pair`'s
/// preserved place.
const MEMBER_RECORD_SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Holder [copy] { inner: Pair; }
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let pair: Pair = Pair { a: s, b: s };
            let copy: Pair = pair;
            let holder: Holder = Holder { inner: copy };
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(s)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Same component shape, but `step` advances `s` on the back edge: the `pair`
/// establishment's scalar fields read a genuinely loop-carried member
/// parameter so it stays inside, and with `pair`'s root uncovered the
/// `holder` copy's member parameter resolves loop-carried — it stays inside
/// too.
const CARRIED_FIELD_RECORD_SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Holder [copy] { inner: Pair; }
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let pair: Pair = Pair { a: s, b: s };
            let copy: Pair = pair;
            let holder: Holder = Holder { inner: copy };
            transition pending > 0 {
                true -> scan(s + 1, pending - 1)
                _ -> finish(s)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

#[test]
fn carried_field_record_establishments_stay_inside() {
    let session = lowered_session(CARRIED_FIELD_RECORD_SOURCE, "carried field record loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_record_establishments(function, component);
    assert_eq!(records.len(), 2, "two member record establishments");
    let record_operations = records
        .iter()
        .map(|(_, node)| operation_of(node))
        .collect::<Vec<_>>();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    for operation in record_operations {
        assert!(
            candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().psi_operation() != operation),
            "a record reading a carried field — or copying a root the run never covers — stays inside"
        );
    }
}

#[test]
fn forged_record_field_copy_root_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_RECORD_SOURCE, "member record loop");
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
    let records = member_record_establishments(function, component);
    let [(_, pair), (_, holder)] = records.as_slice() else {
        panic!("two member record establishments")
    };
    let holder_place = match &holder.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let pair_place = match &pair.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let holder_operation = operation_of(holder);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == holder_operation)
        .expect("the holder establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Swapping the copied root to the holder's own declared place is not a
    // planned rewrite — the seed-derived admission rebinds the member
    // parameter to the run-covered pair place, so the replayed operation
    // comparison rejects the forged spelling.
    let forged = find_operation_mut(&mut unit, holder_operation);
    if let AbstractOperation::EstablishRecord { fields, .. } = &mut forged.operation {
        let terminal_psi::RecordFieldValue::Structural(argument) = &mut fields[0].value else {
            panic!("holder's field copies a place root")
        };
        assert_eq!(argument.place, pair_place);
        argument.place = holder_place;
    }
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

#[test]
fn forged_scalar_array_element_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_ARRAY_SOURCE,
        "member scalar-array loop",
        "scan",
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
    let arrays = member_scalar_array_establishments(function, component);
    let [(_, array)] = arrays.as_slice() else {
        panic!("one member scalar-array establishment")
    };
    let (array_operation, member_element) = match &array.operation {
        AbstractOperation::EstablishScalarArray {
            psi_operation,
            elements,
            ..
        } => (*psi_operation, elements[0]),
        operation => panic!("the member node is an array establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == array_operation)
        .expect("the array establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved elements back to the member parameter skips the
    // seed-derived substitution — the establishment's elements rebind to the
    // preheader anchor, so the replayed operation comparison rejects the
    // drifted spelling.
    let forged = find_operation_mut(&mut unit, array_operation);
    if let AbstractOperation::EstablishScalarArray { elements, .. } = &mut forged.operation {
        elements.fill(member_element);
    }
    for value_use in &mut forged.uses {
        value_use.value = member_element;
    }
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

/// Two-state cycle whose `step` member establishes one affine sum — `picked`
/// holds `Step::More { rest: s }` — and dispatches it in the same block, the
/// exact shape the cyclic-eligibility fence admits. `s` is carried
/// unchanged through `check`'s own `s` parameter back onto `step`'s back
/// edge, so the field operand resolves to the machine's `scale` anchor and
/// the establishment relocates; `check` is the only exit source and `step`
/// dominates it, keeping the move inside the non-speculative gate. The
/// relocation is the family's first custody-rewriting one: the persistent
/// preheader result stays live across traversals, so the dispatch's
/// member-internal edges stop discarding it while `check`'s exit edge to
/// `finish` disposes it instead.
const MEMBER_SCALAR_CASE_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = Step::More { rest: s };
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

#[test]
fn dropped_exit_scalar_case_disposal_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    // The exit edge is a member terminator edge departing the roster —
    // `check`'s `finish` arm — which must dispose the persistent result.
    let exit = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .flat_map(|node| node.successors.iter())
        .find(|edge| !member_targets.contains(&edge.target))
        .expect("the component has an exit edge");
    let exit_edge = exit.psi_edge;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // The applied transform disposes `picked` on the exit edge. Forging the
    // edge back to the seed's empty roster leaves the persistent place live
    // outside the component — the freeze replay's normalized custody expects
    // the disposal, so the drop rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if edge.psi_edge == exit_edge
                        && let Some(index) = edge
                            .trivial_affine_discards
                            .iter()
                            .position(|place| *place == picked)
                    {
                        edge.trivial_affine_discards.remove(index);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        trivial_affine_discards,
                        ..
                    } if node
                        .successors
                        .first()
                        .is_some_and(|edge| edge.psi_edge == exit_edge) =>
                    {
                        trivial_affine_discards.retain(|place| *place != picked);
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if successor.psi_edge == exit_edge {
                                forged |= !successor.trivial_affine_discards.is_empty();
                                successor
                                    .trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    AbstractOperation::StructuralCase { cases, .. } => {
                        for case in cases {
                            if case.psi_edge == exit_edge {
                                forged |= !case.trivial_affine_discards.is_empty();
                                case.trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(
        forged,
        "the exit edge carried the relocated result's disposal"
    );
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

/// Two-state cycle whose `step` member establishes `marker`, the
/// field-free affine record the composed-control lowering emits for a
/// trivial affine local: the cyclic eligibility fence already confined the
/// fresh place to `step`'s member block, discarding it on every departing
/// edge, and `step` dominates both exit sources — so the establishment
/// relocates into the unique preheader while the retained member edges keep
/// the one persistent place live inside the roster and every exit edge
/// disposes it instead.
const MEMBER_AFFINE_RECORD_SOURCE: &str = r#"
    data Root {}
    data Marker {}

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let marker: Marker = Marker {};
            transition pending > 0 {
                true -> check(s, pending - 1)
                _ -> finish(s)
            }
        }
        state check(v: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(v, pending)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Every `EstablishRecord` node inside `component`'s member blocks whose
/// result is affine — the empty-declaration custody-rewriting counterpart of
/// [`member_record_establishments`].
fn member_affine_record_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    member_record_establishments(function, component)
        .into_iter()
        .filter(|(_, node)| {
            matches!(
                &node.operation,
                AbstractOperation::EstablishRecord { result, .. }
                    if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            )
        })
        .collect()
}

#[test]
fn affine_empty_record_establishment_relocates_re_expressing_disposal_custody() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let (record_operation, picked) = match &establishment.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
            ..
        } => {
            assert!(
                fields.is_empty(),
                "the composed-control affine record declares no fields"
            );
            assert_eq!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine,
                "the result is the confined affine place"
            );
            assert!(
                result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty(),
                "the affine result is claim-free"
            );
            (*psi_operation, result.place)
        }
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    // The seed disposes the fresh affine place through member-roster discard
    // rosters — the custody the relocation re-expresses — while no edge
    // departing a non-member block ever spells it.
    let (member_roster_discards, foreign_spellings): (usize, usize) = function
        .blocks
        .iter()
        .flat_map(|block| {
            let member = member_targets.contains(&block.id);
            block
                .nodes
                .iter()
                .flat_map(move |node| node.successors.iter().map(move |edge| (member, edge)))
        })
        .fold((0, 0), |(member_discards, foreign), (member, edge)| {
            let spells = edge.trivial_affine_discards.contains(&picked)
                || edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == picked)
                || edge
                    .structural_bindings
                    .iter()
                    .any(|binding| binding.parameter == picked || binding.argument.place == picked);
            match (member, spells) {
                (true, true) => (member_discards + 1, foreign),
                (false, true) => (member_discards, foreign + 1),
                _ => (member_discards, foreign),
            }
        });
    assert!(
        member_roster_discards > 0,
        "the seed's member edges discard the fresh affine record"
    );
    assert_eq!(
        foreign_spellings, 0,
        "the affine place stays inside the member roster"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == record_operation)
        .expect("the affine empty-record establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the affine empty-record establishment relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    assert_eq!(
        result.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine,
        "the declared multiplicity is byte-exact"
    );
    assert!(
        relocation.node().operand_rewrites().is_empty()
            && relocation.node().argument_rewrites().is_empty(),
        "the empty declaration carries no operand or root rewrites"
    );
    assert_eq!(relocation.destination().block, entry.source);

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let destination = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            assert!(fields.is_empty(), "the moved declaration stays empty");
        }
        operation => panic!("relocated node keeps its record operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The retained member edges keep the persistent result live across
    // member-internal hops and dispose it on every component exit — the
    // affine place survives every traversal and is discarded exactly once on
    // the way out.
    let internal_discards: Vec<_> = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| member_targets.contains(&edge.target))
        .map(|edge| edge.trivial_affine_discards.contains(&picked))
        .collect();
    assert!(
        !internal_discards.is_empty() && internal_discards.iter().all(|discard| !*discard),
        "every member-internal edge keeps the persistent result live"
    );
    let exit_discards: Vec<_> = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| !member_targets.contains(&edge.target))
        .map(|edge| edge.trivial_affine_discards.contains(&picked))
        .collect();
    assert!(
        !exit_discards.is_empty() && exit_discards.iter().all(|discard| *discard),
        "every member exit edge disposes the persistent result exactly once"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

/// Re-spell `scan`'s member `marker` establishment into the direct
/// `EstablishTrivialAffineLocal` form the ordinary checked-machine
/// statement path emits. Source spelling can never produce that op inside a
/// cycle — composed-control lowering spells the same declaration as an
/// empty `EstablishRecord` — so the seed's terminal module is edited to the
/// admitted cyclic shape directly: the declared place is identical, and
/// only the operation kind, its `Unit` result, and the place's declaration
/// kind change.
fn respell_member_record_as_trivial_affine_local(
    module: &mut terminal_psi::TerminalModule,
) -> terminal_psi::StructuralPlaceDeclaration {
    let entry = module.entry;
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .expect("entry scan");
    let (block_index, operation_index, picked) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .operations
                .iter()
                .enumerate()
                .find_map(|(operation_index, operation)| {
                    if !matches!(
                        &operation.kind,
                        terminal_psi::OperationKind::EstablishRecord { fields }
                            if fields.is_empty()
                    ) {
                        return None;
                    }
                    operation
                        .result
                        .structural()
                        .filter(|result| {
                            result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        })
                        .map(|result| (block_index, operation_index, result.place))
                })
        })
        .expect("the cyclic body retains one empty affine establishment");
    let structural_type = machine
        .structural_places
        .iter()
        .find(|place| place.id == picked)
        .and_then(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::OperationResult {
                structural_type, ..
            } => Some(structural_type),
            _ => None,
        })
        .expect("the member establishment's result place declares its type");
    machine.blocks[block_index].operations[operation_index].kind =
        terminal_psi::OperationKind::EstablishTrivialAffineLocal {
            destination: picked,
        };
    machine.blocks[block_index].operations[operation_index].result =
        terminal_psi::OperationResult::Unit;
    let declaration = machine
        .structural_places
        .iter_mut()
        .find(|place| place.id == picked)
        .expect("the declared place exists");
    declaration.kind = semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal: 0,
        structural_type,
        construction: None,
    };
    *declaration
}

/// Build the relocated session over `MEMBER_AFFINE_RECORD_SOURCE` respelled
/// so the member marker lowers as a direct `EstablishTrivialAffineLocal`:
/// apply the one planned candidate and return the optimization input, the
/// transformed unit, the component's member set, the seed's frontier
/// catalog, the relocated place, and the machine.
fn applied_trivial_affine_local_relocation() -> (
    terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    PsiOptimizationUnit,
    std::collections::BTreeSet<semantic_vocabulary::BlockId>,
    Vec<optimization_unit::OwnershipFrontierFact>,
    semantic_vocabulary::PlaceId,
    semantic_vocabulary::MachineId,
) {
    let session = lowered_session_entry_with_module_edit(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member trivial affine local loop",
        "Root::scan",
        |module| {
            respell_member_record_as_trivial_affine_local(module);
        },
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let mut picked = None;
    for block in &function.blocks {
        if !member_targets.contains(&block.id) {
            continue;
        }
        for node in &block.nodes {
            if let AbstractOperation::EstablishTrivialAffineLocal { place, .. } = &node.operation {
                assert!(picked.is_none(), "one member trivial affine local");
                picked = Some(place.id);
            }
        }
    }
    let picked = picked.expect("the member retains the establishment");
    let seed_frontier_facts = session.unit().ownership_frontier_facts.clone();
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, unit) = applied.into_session().into_parts();
    (
        input,
        unit,
        member_targets,
        seed_frontier_facts,
        picked,
        machine,
    )
}

#[test]
fn stale_trivial_affine_local_frontier_catalog_is_rejected() {
    let (input, mut unit, _, seed_frontier_facts, _, machine) =
        applied_trivial_affine_local_relocation();
    // Forging the frontier catalog back to the seed's spelling leaves the
    // affine-authority replay reading custody the transformed edges no
    // longer execute — internal edges keep the persistent place live where
    // the stale catalog still ends it at dispatch — so the stale
    // membership rejects before the catalog comparison is even reached.
    unit.ownership_frontier_facts = seed_frontier_facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

/// A member scalar case lent to a unit call: `step` establishes the affine
/// `picked` from the invariant `s`, shared-borrows it into `weigh`, then
/// dispatches it. The borrow loans the place for the call's duration only, so
/// it is an observation position inside the containment bound: `picked`
/// relocates into the preheader, and the pure `weigh(&picked, s)` follows it
/// through the same run because its borrowed root is now run-produced and its
/// scalar argument resolves to `scale`'s anchor.
const SHARED_BORROW_SCALAR_CASE_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine weigh(step: &Step, bias: u32) {}

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = Step::More { rest: s };
            weigh(&picked, s);
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Same shape, but the borrowing call also reads the loop-carried
/// countdown: the call stays inside, and the relocated `picked` reaches it as
/// the one persistent preheader place every traversal lends.
const CARRIED_BORROWER_SCALAR_CASE_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine weigh(step: &Step, bias: u32 [0..=5]) {}

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = Step::More { rest: s };
            weigh(&picked, pending);
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// The member Unit-result `CallStructural` lending `picked` and its structural
/// argument access.
fn member_borrowing_unit_call(
    function: &optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    picked: semantic_vocabulary::PlaceId,
) -> (
    semantic_vocabulary::OperationId,
    terminal_psi::StructuralAccess,
) {
    let calls = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .filter(move |block| block.id == *member)
                .flat_map(|block| block.nodes.iter())
        })
        .filter_map(|node| match &node.operation {
            AbstractOperation::CallStructural {
                psi_operation,
                structural_arguments,
                result: AbstractStructuralCallResult::Unit,
                ..
            } => structural_arguments
                .iter()
                .find(|argument| argument.place == picked)
                .map(|argument| (*psi_operation, argument.access)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one member unit call lends the scalar case: {calls:?}")
    };
    *call
}

/// Propose, validate and apply the one candidate, require the applied session
/// to be a fixed point that replays through the freeze fence, and return the
/// relocated operations in run order.
fn relocate_single_candidate(
    session: crate::VerifiedPsiOptimizationSession,
) -> Vec<semantic_vocabulary::OperationId> {
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocated = candidate
        .relocations()
        .iter()
        .map(|relocation| relocation.node().psi_operation())
        .collect::<Vec<_>>();
    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
    let (input, unit) = applied.into_session().into_parts();
    crate::validation::validate_transformed_psi_optimization_unit(&input, &unit)
        .expect("the transformed unit replays through the freeze fence");
    relocated
}

#[test]
fn shared_borrow_unit_call_relocates_behind_its_scalar_case() {
    let session = lowered_session_entry(
        SHARED_BORROW_SCALAR_CASE_SOURCE,
        "shared-borrow scalar-case loop",
        "Root::scan",
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
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let establishment_operation = operation_of(establishment);
    let (call_operation, access) = member_borrowing_unit_call(function, component, picked);
    assert_eq!(access, terminal_psi::StructuralAccess::SharedBorrow);

    let relocated = relocate_single_candidate(session);
    let establishment_position = relocated
        .iter()
        .position(|operation| *operation == establishment_operation)
        .expect("the lent scalar case relocates");
    let call_position = relocated
        .iter()
        .position(|operation| *operation == call_operation)
        .expect("the pure borrowing call relocates behind it");
    assert!(
        establishment_position < call_position,
        "the run orders the borrowed root's producer ahead of the borrower"
    );
}

#[test]
fn carried_borrower_stays_while_its_scalar_case_relocates() {
    let session = lowered_session_entry(
        CARRIED_BORROWER_SCALAR_CASE_SOURCE,
        "carried-borrower scalar-case loop",
        "Root::scan",
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
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let establishment_operation = operation_of(establishment);
    let (call_operation, access) = member_borrowing_unit_call(function, component, picked);
    assert_eq!(access, terminal_psi::StructuralAccess::SharedBorrow);
    assert!(
        crate::validation::relocation_rewrites::scalar_case_result_contained(
            function, component, picked
        ),
        "a claim-free shared loan keeps the case inside the containment bound"
    );

    let relocated = relocate_single_candidate(session);
    assert!(
        relocated.contains(&establishment_operation),
        "the lent scalar case relocates"
    );
    assert!(
        !relocated.contains(&call_operation),
        "the carried-argument borrower stays inside, lending the preheader place"
    );
}

#[test]
fn writing_or_moving_loans_leave_the_containment_bound() {
    let session = lowered_session_entry(
        SHARED_BORROW_SCALAR_CASE_SOURCE,
        "shared-borrow scalar-case loop",
        "Root::scan",
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
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let (call_operation, _) = member_borrowing_unit_call(function, component, picked);
    assert!(
        crate::validation::relocation_rewrites::scalar_case_result_contained(
            function, component, picked
        )
    );
    // The Psi lowering does not yet produce a writing or moving loan of a
    // member-produced case inside a cycle, so the controls respell the
    // verified call: a mutable or write-only borrow lets the callee write the
    // place a persistent preheader result would carry into the next
    // traversal, and an owned argument moves it outright.
    for access in [
        terminal_psi::StructuralAccess::MutableBorrow,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
        terminal_psi::StructuralAccess::Owned,
    ] {
        let mut respelled = function.clone();
        let node = respelled
            .blocks
            .iter_mut()
            .flat_map(|block| block.nodes.iter_mut())
            .find(|node| {
                matches!(
                    &node.operation,
                    AbstractOperation::CallStructural {
                        psi_operation,
                        result: AbstractStructuralCallResult::Unit,
                        ..
                    }
                        if *psi_operation == call_operation
                )
            })
            .expect("the borrowing call exists");
        let AbstractOperation::CallStructural {
            structural_arguments,
            result: AbstractStructuralCallResult::Unit,
            ..
        } = &mut node.operation
        else {
            panic!("the borrowing node is a unit call")
        };
        for argument in structural_arguments.iter_mut() {
            if argument.place == picked {
                argument.access = access;
            }
        }
        assert!(
            !crate::validation::relocation_rewrites::scalar_case_result_contained(
                &respelled, component, picked
            ),
            "a {access:?} loan leaves the containment bound"
        );
    }
}
