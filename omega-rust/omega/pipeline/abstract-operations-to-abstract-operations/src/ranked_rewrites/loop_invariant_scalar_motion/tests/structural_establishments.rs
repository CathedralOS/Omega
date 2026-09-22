//! Loop-invariant structural establishments: records, scalar arrays,
//! scalar cases, affine records and trivial affine locals, and the disposal
//! custody a relocated establishment re-expresses.

use crate::{
    LoopInvariantNodeResult, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
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

/// Same cycle shape as `BYPASSED_MEMBER_SOURCE`, but `step` establishes the
/// record: the entry state's `done` arm can leave the component before `step`
/// ever runs, so the establishment is speculation and stays inside while the
/// entry state's own invariant leaves still relocate.
const BYPASSED_RECORD_SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let pair: Pair = Pair { a: s, b: s };
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(s)
            }
        }
        state done() {}
        state finish(r: u32 in Wrapping) {}
    }
"#;

#[test]
fn invariant_record_establishments_relocate_preserving_their_places() {
    let session = lowered_session(MEMBER_RECORD_SOURCE, "member record loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_record_establishments(function, component);
    let [(_, pair), (_, holder)] = records.as_slice() else {
        panic!("two member record establishments")
    };
    let (pair_operation, pair_place, pair_fields) = match &pair.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
        } => (*psi_operation, result.place, fields),
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let (holder_operation, holder_place, copied_root) = match &holder.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
        } => {
            let [field] = fields.as_slice() else {
                panic!("holder declares one field")
            };
            let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                panic!("holder's field copies a place root")
            };
            (*psi_operation, result.place, argument.place)
        }
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    assert_ne!(
        copied_root, pair_place,
        "the holder copies the copy block's member structural parameter"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let pair_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == pair_operation)
        .expect("the pair establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = pair_relocation.node().result() else {
        panic!("the record establishment relocates its structural result")
    };
    assert_eq!(result.place, pair_place, "the declared place is byte-exact");
    // Both scalar fields resolve to the same `scale` representative — the
    // substitution rewrites every spelled field value to the preheader anchor.
    let rewrites = pair_relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        pair_fields.len(),
        "every scalar field value carries one member-parameter rewrite"
    );
    let representative = rewrites[0].1;
    for (rewritten, field) in rewrites.iter().zip(pair_fields.iter()) {
        let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value else {
            panic!("pair's fields are scalar")
        };
        assert_eq!(rewritten.0, *value, "the rewrite spells the field value");
        assert_eq!(
            rewritten.1, representative,
            "every field rewrites to the same preheader anchor"
        );
    }
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == representative)
        .expect("the field representative is the machine's `scale` parameter")
        .value;

    let holder_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == holder_operation)
        .expect("the holder establishment is a planned relocation");
    assert_eq!(
        holder_relocation.node().argument_rewrites(),
        &[(copied_root, pair_place)],
        "the moved copy rebinds its member parameter root to the run-covered record place"
    );
    assert_eq!(pair_relocation.destination().block, entry.source);
    assert_eq!(holder_relocation.destination().block, entry.source);
    assert!(
        pair_relocation.destination().node < holder_relocation.destination().node,
        "the relocated pair lands ahead of the holder copying its root"
    );

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
        .find(|block| block.id == pair_relocation.destination().block)
        .expect("destination block exists");
    let moved_pair =
        &destination.nodes[usize::try_from(pair_relocation.destination().node).unwrap()];
    match &moved_pair.operation {
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            assert_eq!(result.place, pair_place, "the declared place is byte-exact");
            for field in fields {
                let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value else {
                    panic!("pair's fields are scalar")
                };
                assert_eq!(
                    *value, anchor,
                    "the moved initializer rebinds to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its record operation: {operation:?}"),
    }
    let moved_holder =
        &destination.nodes[usize::try_from(holder_relocation.destination().node).unwrap()];
    match &moved_holder.operation {
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            assert_eq!(
                result.place, holder_place,
                "the declared place is byte-exact"
            );
            let [field] = fields.as_slice() else {
                panic!("holder declares one field")
            };
            let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                panic!("holder's field copies a place root")
            };
            assert_eq!(
                argument.place, pair_place,
                "the moved copy spells the relocated pair's preserved place"
            );
        }
        operation => panic!("relocated node keeps its record operation: {operation:?}"),
    }
    assert_eq!(moved_pair.provenance, pair_relocation.node().provenance());
    assert_eq!(moved_pair.fuel, pair_relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

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
fn bypassed_member_record_establishment_stays_inside() {
    let session = lowered_session(BYPASSED_RECORD_SOURCE, "bypassed record loop");
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
    assert_eq!(records.len(), 1, "one member record establishment");
    let record_operation = operation_of(records[0].1);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != record_operation),
        "a record establishment in a member the exit can bypass is speculation"
    );
    assert!(
        !candidate.relocations().is_empty(),
        "the entry state's invariant leaves still relocate"
    );
}

#[test]
fn forged_record_field_initializer_is_rejected_by_the_freeze_fence() {
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
    let [(_, pair), _] = records.as_slice() else {
        panic!("two member record establishments")
    };
    let (pair_operation, member_field_value) = match &pair.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            fields,
            ..
        } => {
            let terminal_psi::RecordFieldValue::Scalar { value, .. } = fields[0].value else {
                panic!("pair's fields are scalar")
            };
            (*psi_operation, value)
        }
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == pair_operation)
        .expect("the pair establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved field initializer back to the member parameter skips
    // the seed-derived substitution — the establishment's field values rebind
    // to the preheader anchor, so the replayed operation comparison rejects
    // the drifted spelling.
    let forged = find_operation_mut(&mut unit, pair_operation);
    if let AbstractOperation::EstablishRecord { fields, .. } = &mut forged.operation {
        for field in fields {
            if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                *value = member_field_value;
            }
        }
    }
    for value_use in &mut forged.uses {
        value_use.value = member_field_value;
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

/// Same component shape, but the establishment reads the loop-carried
/// `remaining` countdown: the member parameter never resolves to a preheader
/// representative, so the establishment stays inside even though its member
/// block is guaranteed to execute.
const CARRIED_ELEMENT_SCALAR_ARRAY_SOURCE: &str = r#"
    machine first(row: [u64; 2]) -> u64 { 0 }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first([remaining, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

#[test]
fn invariant_scalar_array_and_its_owned_argument_call_relocate_together() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_ARRAY_SOURCE,
        "member scalar-array loop",
        "scan",
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
    let arrays = member_scalar_array_establishments(function, component);
    let [(_, array)] = arrays.as_slice() else {
        panic!("one member scalar-array establishment")
    };
    let (array_operation, array_place, elements) = match &array.operation {
        AbstractOperation::EstablishScalarArray {
            psi_operation,
            result,
            elements,
        } => (*psi_operation, result.place, elements),
        operation => panic!("the member node is an array establishment: {operation:?}"),
    };
    assert_eq!(elements.len(), 2, "the array declares two scalar elements");
    // The consuming call spells the fresh array root as an `Owned`
    // argument: the unrestricted payload copies into the callee, so the
    // argument is the observation a shared borrow is, and the call
    // relocates behind its producer in the same run.
    let (call_operation, call_result) = component
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
        .find_map(|node| match &node.operation {
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                ..
            } => Some((*psi_operation, result.value)),
            _ => None,
        })
        .expect("the member block holds the owned-argument scalar call");

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let array_position = candidate
        .relocations()
        .iter()
        .position(|relocation| relocation.node().psi_operation() == array_operation)
        .expect("the array establishment is a planned relocation");
    let call_position = candidate
        .relocations()
        .iter()
        .position(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the owned-argument call relocates behind its producer");
    assert!(
        array_position < call_position,
        "the run orders the producer ahead of the owned-argument call"
    );
    let relocation = &candidate.relocations()[array_position];
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the array establishment relocates its structural result")
    };
    assert_eq!(
        result.place, array_place,
        "the declared place is byte-exact"
    );
    // Both elements resolve to the same `scale` representative — the
    // substitution rewrites every spelled element to the preheader anchor.
    let rewrites = relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        elements.len(),
        "every element carries one member-parameter rewrite"
    );
    let representative = rewrites[0].1;
    for (rewritten, element) in rewrites.iter().zip(elements.iter()) {
        assert_eq!(
            rewritten.0, *element,
            "the rewrite spells the element value"
        );
        assert_eq!(
            rewritten.1, representative,
            "every element rewrites to the same preheader anchor"
        );
    }
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == representative)
        .expect("the element representative is the machine's `scale` parameter")
        .value;
    // The call's `Owned` argument names the member-produced root its
    // producer already covered: the run keeps the declared place identity
    // byte-exact, so the relocated call needs no argument rewrite, and its
    // scalar result joins the run's relocated values.
    let call_relocation = &candidate.relocations()[call_position];
    let LoopInvariantNodeResult::Scalar { value, .. } = call_relocation.node().result() else {
        panic!("the owned-argument call relocates its scalar result")
    };
    assert_eq!(
        *value, call_result,
        "the relocated call preserves its scalar result identity"
    );
    assert!(
        call_relocation.node().argument_rewrites().is_empty(),
        "the run-covered root keeps the argument byte-exact"
    );
    assert_eq!(call_relocation.destination().block, entry.source);

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
        AbstractOperation::EstablishScalarArray {
            result, elements, ..
        } => {
            assert_eq!(
                result.place, array_place,
                "the declared place is byte-exact"
            );
            for element in elements {
                assert_eq!(
                    *element, anchor,
                    "the moved element rebinds to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its array operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The relocated call still copies the persistent place — `Owned` access
    // and the declared root move byte-exact inside the operation.
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &moved_call.operation
    else {
        panic!("the relocated node keeps its call operation")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("the owned-argument call carries one structural argument")
    };
    assert_eq!(
        argument.access,
        terminal_psi::StructuralAccess::Owned,
        "the moved call keeps its owned access spelling"
    );
    assert_eq!(
        argument.place, array_place,
        "the moved call copies the relocated array's preserved place"
    );
    assert_eq!(moved_call.provenance, call_relocation.node().provenance());
    assert!(
        applied
            .session()
            .unit()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .filter(|block| member_targets.contains(&block.id))
            .flat_map(|block| &block.nodes)
            .all(|node| {
                !matches!(
                    node.operation,
                    AbstractOperation::CallStructuralScalar { .. }
                        | AbstractOperation::EstablishScalarArray { .. }
                )
            }),
        "the member roster keeps neither the establishment nor its consuming call"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_element_scalar_array_establishment_stays_inside() {
    let session = lowered_session_entry(
        CARRIED_ELEMENT_SCALAR_ARRAY_SOURCE,
        "carried element scalar-array loop",
        "scan",
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
    let arrays = member_scalar_array_establishments(function, component);
    let [(_, array)] = arrays.as_slice() else {
        panic!("one member scalar-array establishment")
    };
    let array_operation = operation_of(array);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != array_operation),
        "an array establishment reading a carried element stays inside"
    );
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
        for element in elements {
            *element = member_element;
        }
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

/// Same component shape, but the case field reads the loop-carried
/// `pending` countdown: the member parameter never resolves to a preheader
/// representative, so the establishment stays inside even though its member
/// block is guaranteed to execute.
const CARRIED_FIELD_SCALAR_CASE_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = Step::More { rest: pending };
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
fn invariant_scalar_case_establishment_relocates_re_expressing_dispatch_custody() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
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
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let (case_operation, picked, fields) = match &establishment.operation {
        AbstractOperation::EstablishScalarCase {
            psi_operation,
            result,
            fields,
            ..
        } => (*psi_operation, result.place, fields),
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    assert_eq!(fields.len(), 1, "the case declares one scalar field");
    // The seed's dispatch discards the fresh affine place on every case
    // edge — the custody the relocation re-expresses.
    let dispatch = component
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
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the member block holds the dispatch on the fresh sum");
    let AbstractOperation::StructuralCase { cases, .. } = &dispatch.operation else {
        panic!("the dispatch is a structural case")
    };
    for case in cases {
        assert!(
            case.trivial_affine_discards.contains(&picked),
            "the seed's dispatch edge discards the fresh affine sum"
        );
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == case_operation)
        .expect("the scalar-case establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the scalar-case establishment relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    let rewrites = relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        1,
        "the single field carries one member-parameter rewrite"
    );
    assert_eq!(
        rewrites[0].0, fields[0].value,
        "the rewrite spells the field value"
    );
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == rewrites[0].1)
        .expect("the field representative is the machine's `scale` parameter")
        .value;
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
        AbstractOperation::EstablishScalarCase { result, fields, .. } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            for field in fields {
                assert_eq!(
                    field.value, anchor,
                    "the moved field rebinds to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its case operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The retained dispatch keeps the persistent result live on
    // member-internal edges and disposes it on the component's exit: the
    // affine place survives every traversal's case inspection and is
    // discarded exactly once on the way out.
    let staying_dispatch = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the dispatch survives in the member block");
    for edge in &staying_dispatch.successors {
        assert!(
            !edge.trivial_affine_discards.contains(&picked),
            "a member-bound dispatch edge keeps the persistent result live"
        );
    }
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

#[test]
fn carried_field_scalar_case_establishment_stays_inside() {
    let session = lowered_session_entry(
        CARRIED_FIELD_SCALAR_CASE_SOURCE,
        "carried field scalar-case loop",
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
    let case_operation = operation_of(establishment);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != case_operation),
        "a scalar-case establishment reading a carried field stays inside"
    );
}

#[test]
fn forged_scalar_case_field_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
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
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let (case_operation, member_field) = match &establishment.operation {
        AbstractOperation::EstablishScalarCase {
            psi_operation,
            fields,
            ..
        } => (*psi_operation, fields[0].value),
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == case_operation)
        .expect("the scalar-case establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved field back to the member parameter skips the
    // seed-derived substitution — the establishment's field rebinds to the
    // preheader anchor, so the replayed operation comparison rejects the
    // drifted spelling.
    let forged = find_operation_mut(&mut unit, case_operation);
    if let AbstractOperation::EstablishScalarCase { fields, .. } = &mut forged.operation {
        for field in fields {
            field.value = member_field;
        }
    }
    for value_use in &mut forged.uses {
        value_use.value = member_field;
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
fn kept_internal_scalar_case_discard_is_rejected_by_the_freeze_fence() {
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
        AbstractOperation::EstablishScalarCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let dispatch_block = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
        })
        .find(|block| {
            block.nodes.iter().any(|node| {
                matches!(
                    &node.operation,
                    AbstractOperation::StructuralCase { source, .. } if *source == picked
                )
            })
        })
        .expect("the dispatch's member block")
        .id;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the dispatch's member-internal edges to keep discarding the
    // persistent result restores the source's per-traversal custody: the
    // first traversal would end the one preheader place the next traversal
    // dispatches. The freeze replay normalizes the seed's retained node
    // through the same custody rewrite — internal edges stripped, exits
    // disposing — so the kept discard rejects byte-exact.
    let forged = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the dispatch survives in the member block");
    if let AbstractOperation::StructuralCase { cases, .. } = &mut forged.operation {
        for case in cases {
            if member_targets.contains(&case.target)
                && !case.trivial_affine_discards.contains(&picked)
            {
                case.trivial_affine_discards.push(picked);
            }
        }
    }
    for edge in &mut forged.successors {
        if member_targets.contains(&edge.target) && !edge.trivial_affine_discards.contains(&picked)
        {
            edge.trivial_affine_discards.push(picked);
        }
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == dispatch_block
    ));
}

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
        AbstractOperation::EstablishScalarCase { result, .. } => result.place,
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

#[test]
fn stale_scalar_case_frontier_catalog_is_rejected() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    // The seed catalog still carries the source's per-traversal custody:
    // `picked` owned between its member establishment and the dispatch-edge
    // discards, dead everywhere else.
    let seed_frontier_facts = session.unit().ownership_frontier_facts.clone();
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
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

/// The component's unique entry edge is one of two successors on the
/// preheader's terminator: reaching the preheader does not guarantee
/// entering the loop, so relocating the `marker` establishment would execute
/// it on traversals that never enter the component. The establishment stays
/// inside; work-free scalar-constant leaves still relocate.
const CONDITIONAL_ENTRY_AFFINE_RECORD_SOURCE: &str = r#"
    data Root {}
    data Marker {}

    machine Root::enter(scale: u32, go: bool, remaining: u32 [0..=5])
    {
        transition go {
            true -> scan(scale, remaining)
            _ -> done()
        }
        state scan(s: u32, pending: u32 [0..=5]) {
            let marker: Marker = Marker {};
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(s)
            }
        }
        state done() {}
        state finish(r: u32) {}
    }
"#;

/// Same two-state cycle, but the empty affine record is established inside
/// `check`: a traversal can leave through `step`'s own `finish` arm without
/// ever reaching `check`, so hoisting the establishment would speculate the
/// work and the member stays inside.
const BYPASSED_AFFINE_RECORD_SOURCE: &str = r#"
    data Root {}
    data Marker {}

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> check(s, pending - 1)
                _ -> finish(s)
            }
        }
        state check(v: u32, pending: u32 [0..=5]) {
            let marker: Marker = Marker {};
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

/// The direct `EstablishTrivialAffineLocal` spelling relocates under the
/// same custody contract as the composed record form: the relocation's
/// declared result is the persistent place itself, member-internal edges
/// keep it live, and every component exit disposes it exactly once.
#[test]
fn trivial_affine_local_establishment_relocates_re_expressing_disposal_custody() {
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
    let establishments = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
                .nodes
                .iter()
        })
        .filter(|node| {
            matches!(
                &node.operation,
                AbstractOperation::EstablishTrivialAffineLocal { .. }
            )
        })
        .collect::<Vec<_>>();
    let [establishment] = establishments.as_slice() else {
        panic!("one member trivial-affine-local establishment")
    };
    let (local_operation, picked) = match &establishment.operation {
        AbstractOperation::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            ..
        } => (*psi_operation, place.id),
        operation => panic!("the member node is the direct establishment: {operation:?}"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the trivial affine local establishment is a planned relocation");
    let LoopInvariantNodeResult::TrivialAffineLocal(place) = relocation.node().result() else {
        panic!("the trivial affine local relocation names its declared place")
    };
    assert_eq!(place.id, picked, "the declared place is byte-exact");
    assert!(
        relocation.node().operand_rewrites().is_empty()
            && relocation.node().argument_rewrites().is_empty(),
        "the unit-result establishment carries no operand or root rewrites"
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
        AbstractOperation::EstablishTrivialAffineLocal { place, .. } => {
            assert_eq!(place.id, picked, "the declared place is byte-exact");
        }
        operation => panic!("relocated node keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The persistent place stays live across member-internal hops and is
    // disposed exactly once on every component exit — the custody the
    // relocation re-expressed rather than retained.
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
        "every member-internal edge keeps the persistent place live"
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
        "every member exit edge disposes the persistent place exactly once"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
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
fn kept_internal_trivial_affine_local_discard_is_rejected_by_the_freeze_fence() {
    let (input, mut unit, member_targets, _, picked, machine) =
        applied_trivial_affine_local_relocation();
    // Forging a member-internal edge to keep discarding the persistent
    // place restores the source's per-traversal custody: the first
    // traversal would end the one preheader place the next traversal still
    // owns. The freeze replay normalizes the seed's retained node through
    // the same custody rewrite — internal edges stripped, exits disposing —
    // so the kept discard rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if member_targets.contains(&edge.target)
                        && !edge.trivial_affine_discards.contains(&picked)
                    {
                        edge.trivial_affine_discards.push(picked);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        target,
                        trivial_affine_discards,
                        ..
                    } if member_targets.contains(target) => {
                        if !trivial_affine_discards.contains(&picked) {
                            trivial_affine_discards.push(picked);
                        }
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if member_targets.contains(&successor.target)
                                && !successor.trivial_affine_discards.contains(&picked)
                            {
                                successor.trivial_affine_discards.push(picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(forged, "a member-internal edge carried the forged discard");
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

#[test]
fn dropped_exit_trivial_affine_local_disposal_is_rejected_by_the_freeze_fence() {
    let (input, mut unit, member_targets, _, picked, machine) =
        applied_trivial_affine_local_relocation();
    // An exit edge — a member terminator edge departing the roster — must
    // dispose the persistent place. Forging it back to the seed's empty
    // roster leaves the place live outside the component; the freeze
    // replay's normalized custody expects the disposal, so the drop
    // rejects byte-exact.
    let exit_edge = unit
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| !member_targets.contains(&edge.target))
        .map(|edge| edge.psi_edge)
        .next()
        .expect("the component has an exit edge");
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
        "the exit edge carried the relocated place's disposal"
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

#[test]
fn conditional_entry_affine_record_establishment_stays_inside() {
    let session = lowered_session_entry(
        CONDITIONAL_ENTRY_AFFINE_RECORD_SOURCE,
        "conditional-entry affine record loop",
        "Root::enter",
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
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let record_operation = operation_of(establishment);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != record_operation),
        "an affine record behind a conditional entry stays inside"
    );
}

#[test]
fn bypassed_member_affine_record_establishment_stays_inside() {
    let session = lowered_session_entry(
        BYPASSED_AFFINE_RECORD_SOURCE,
        "bypassed affine record loop",
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
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let record_operation = operation_of(establishment);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != record_operation),
        "an affine record a bypassing exit can skip stays inside"
    );
}

#[test]
fn forged_affine_record_result_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
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
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let record_operation = operation_of(establishment);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == record_operation)
        .expect("the affine empty-record establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved result's multiplicity to unrestricted drops the
    // affine custody the relocation re-expressed — the replayed operation
    // comparison retains every source-owned field, so the drifted spelling
    // rejects byte-exact.
    let forged = find_operation_mut(&mut unit, record_operation);
    if let AbstractOperation::EstablishRecord { result, .. } = &mut forged.operation {
        result.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
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
fn kept_internal_affine_record_discard_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
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
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging a member-internal edge to keep discarding the persistent
    // result restores the source's per-traversal custody: the first
    // traversal would end the one preheader place the next traversal still
    // owns. The freeze replay normalizes the seed's retained node through
    // the same custody rewrite — internal edges stripped, exits disposing —
    // so the kept discard rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if member_targets.contains(&edge.target)
                        && !edge.trivial_affine_discards.contains(&picked)
                    {
                        edge.trivial_affine_discards.push(picked);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        target,
                        trivial_affine_discards,
                        ..
                    } if member_targets.contains(target) => {
                        if !trivial_affine_discards.contains(&picked) {
                            trivial_affine_discards.push(picked);
                        }
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if member_targets.contains(&successor.target)
                                && !successor.trivial_affine_discards.contains(&picked)
                            {
                                successor.trivial_affine_discards.push(picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(forged, "a member-internal edge carried the forged discard");
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

#[test]
fn dropped_exit_affine_record_disposal_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
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
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
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

#[test]
fn stale_affine_record_frontier_catalog_is_rejected() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    // The seed catalog still carries the source's per-traversal custody:
    // `picked` owned between its member establishment and the member-edge
    // discards, dead everywhere else.
    let seed_frontier_facts = session.unit().ownership_frontier_facts.clone();
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
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
