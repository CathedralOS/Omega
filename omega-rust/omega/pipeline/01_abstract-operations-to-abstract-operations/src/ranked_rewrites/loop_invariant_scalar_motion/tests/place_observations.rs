//! Loop-invariant place observations: place reads over an invariant root,
//! member view parameters resolving across member edges, and member reads
//! of a member-produced root the same run relocates.

use crate::VerifiedPsiOptimizationSession;
use crate::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{NodeLocation, PsiProvenance};
use optimization_unit_semantics::OptimizationUnitValidationError;

use super::{
    lowered_session, lowered_session_entry, member_length_reads, member_primitive_local,
    member_structural_scalar_call, operation_of, refresh_coordinates_and_effects, take_operation,
};

/// A two-member component whose view parameters chain through the internal
/// edge: `step` binds `view` from `entries` on the entry edge and from
/// `swap`'s `other` on the back edge, while `other` binds `step`'s `view` on
/// `step`'s internal edge, so both member view parameters resolve
/// transitively to the `entries` root through each other. `swap` owns the
/// component's only exit, so both members are guaranteed to execute on every
/// traversal that leaves — `view.len` in `step` and `other.len` in `swap`
/// each relocate, rebinding their observed root to `entries`.
const TRANSITIVE_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], pending: u32 [0..=5]) {
            let length: u64 = view.len;
            let doubled: u32 in Wrapping = s + s;
            transition { _ -> swap(s, view, pending) }
        }
        state swap(t: u32 in Wrapping, other: &[u8], pending: u32 [0..=5]) {
            let width: u64 = other.len;
            transition pending > 0 {
                true -> step(t, other, pending - 1)
                _ -> finish(t)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

#[test]
fn member_view_parameters_resolve_across_member_edges() {
    let session = lowered_session(TRANSITIVE_VIEW_SOURCE, "transitive view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-member component")
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
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    let reads = member_length_reads(function, component);
    let [(_, step_read), (_, swap_read)] = reads.as_slice() else {
        panic!("each member observes its view length once")
    };
    // Both member view parameters resolve through each other to `entries`:
    // `step.view` chains `swap.other` and `swap.other` chains `step.view`,
    // with the entry edge's `entries` anchor breaking the tie.
    for (_, read) in &reads {
        assert_eq!(
            crate::validation::place_observations::invariant_place_observation_admission(
                function,
                component,
                read,
                &std::collections::BTreeSet::new(),
            ),
            Some(entries_parameter.place),
            "each member view observation resolves its root to `entries`"
        );
    }
    let read_operations: Vec<_> = [step_read, swap_read]
        .iter()
        .map(|read| match read.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => *operation,
            _ => panic!("each observation carries its operation identity"),
        })
        .collect();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocations: Vec<_> = read_operations
        .iter()
        .map(|operation| {
            candidate
                .relocations()
                .iter()
                .find(|relocation| relocation.node().psi_operation() == *operation)
                .expect("each member view observation is a planned relocation")
        })
        .collect();
    for relocation in &relocations {
        let (parameter, representative) = relocation
            .node()
            .root_rewrite()
            .expect("each relocation carries a root rewrite");
        assert_eq!(representative, entries_parameter.place);
        assert_eq!(
            Some(parameter),
            crate::validation::invariant_operations::admissible_invariant_place_read(
                find_member_node(function, relocation.node().location())
            ),
            "each relocation rebinds its member parameter root to `entries`"
        );
        assert_eq!(relocation.destination().block, preheader);
    }
    let entries_place = entries_parameter.place;

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
    for relocation in relocations {
        let destination = output_function
            .blocks
            .iter()
            .find(|block| block.id == relocation.destination().block)
            .expect("destination block exists");
        let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
        match &moved.operation {
            AbstractOperation::ByteSequenceLength { source, .. } => {
                assert_eq!(*source, entries_place);
            }
            operation => panic!("relocated observation keeps its operation: {operation:?}"),
        }
        assert!(
            output_function
                .blocks
                .iter()
                .find(|block| block.id == relocation.node().location().block)
                .expect("member block exists")
                .nodes
                .iter()
                .all(|node| !matches!(
                    node.operation,
                    AbstractOperation::ByteSequenceLength { .. }
                )),
            "the member's observation exists once, at the destination"
        );
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

fn find_member_node(
    function: &optimization_unit::PsiOptimizationFunction,
    site: NodeLocation,
) -> &optimization_unit::OptimizationNode {
    function
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("member block exists")
        .nodes
        .get(usize::try_from(site.node).unwrap())
        .expect("member node exists")
}

/// A cyclic `let mut` primitive local kept alive by a shared borrow to a
/// pure structural scalar call, then read by a member `PrimitiveScalarRead`:
/// `scratch`'s root is produced inside the component, but the establishment
/// relocates and the run covers the root — so the read relocates behind its
/// producer spelling the same member-produced root byte-exact, the call's
/// shared-borrow argument does the same, and the `measured & observed`
/// computation chained on both results relocates through the scalar path.
const MEMBER_READ_LOCAL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure(&scratch, scale);
        let observed: u64 = scratch;
        let combined: u64 = measured & observed;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// A two-member component whose `swap` member view parameter binds only the
/// subslice `step` establishes fresh each traversal: `view` keeps resolving
/// to the machine's `entries` root through `carry`, while `other`'s single
/// binding edge spells the member-produced subslice root. When the run
/// covers the subslice, `other` resolves to its declared place — so
/// `other.len` relocates out of `swap` rebinding its observed root to the
/// moved subslice's byte-exact result place.
const MEMBER_PRODUCED_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u64, entries: &[u8], remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u64, view: &[u8], pending: u64 [0..=5]) {
            let length: u64 = view.len;
            transition { _ -> swap(s, view[0..view.len], view, pending) }
        }
        state swap(t: u64, other: &[u8], carry: &[u8], rest: u64 [0..=5]) {
            let width: u64 = other.len;
            transition rest > 0 {
                true -> step(t, carry, rest - 1)
                _ -> finish(width)
            }
        }
        state finish(r: u64) {}
    }
"#;

/// The `PrimitiveScalarRead` node inside a member block and its block —
/// the read counterpart of [`member_primitive_local`].
fn member_scalar_read<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::PrimitiveScalarRead { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the primitive-scalar read lives in a member block")
}

#[test]
fn member_read_relocates_with_its_member_produced_root() {
    let session = lowered_session_entry(MEMBER_READ_LOCAL_SOURCE, "member read local loop", "scan");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let (_, read) = member_scalar_read(function, component);
    let (local_operation, local_place) = match &local.operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.place),
        operation => panic!("the member node is a primitive-local establishment: {operation:?}"),
    };
    let call_operation = operation_of(call);
    let (read_operation, read_source, read_result) = match &read.operation {
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, *source, result.value),
        operation => panic!("the member node is a scalar read: {operation:?}"),
    };
    assert_eq!(
        read_source, local_place,
        "the member read observes the local's member-produced root"
    );
    // The admission boundary, both ways: the read's member-produced root is
    // invisible at the preheader and no member parameter resolves it, so an
    // empty run refuses — covering the producer's root admits the read and
    // resolves to the root it already spells.
    assert!(
        crate::validation::place_observations::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        )
        .is_none(),
        "an uncovered member-produced root keeps the read inside"
    );
    assert_eq!(
        crate::validation::place_observations::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::from([local_place]),
        ),
        Some(local_place),
        "the run-covered member-produced root admits the read byte-exact"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let local_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment relocates");
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the shared-borrow call relocates with its producer");
    let read_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the member read relocates behind its producer");
    // The member-produced root is already what the moved operations spell:
    // the producer's declared place identity survives byte-exact, so the read
    // needs no root rewrite and the call no argument rewrite.
    assert!(read_relocation.node().root_rewrite().is_none());
    assert!(call_relocation.node().argument_rewrites().is_empty());
    assert!(
        local_relocation.destination().node < read_relocation.destination().node
            && local_relocation.destination().node < call_relocation.destination().node,
        "the relocated producer lands ahead of both member-produced-root consumers"
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
        .find(|block| block.id == read_relocation.destination().block)
        .expect("destination block exists");
    let moved_read =
        &destination.nodes[usize::try_from(read_relocation.destination().node).unwrap()];
    match &moved_read.operation {
        AbstractOperation::PrimitiveScalarRead { source, result, .. } => {
            assert_eq!(
                *source, local_place,
                "the relocated read keeps spelling the member-produced root"
            );
            assert_eq!(result.value, read_result);
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
fn member_read_moved_without_its_producer_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(MEMBER_READ_LOCAL_SOURCE, "member read local loop", "scan");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    let (read_block, read) = member_scalar_read(function, component);
    let member = read_block.id;
    let preheader = entry.source;
    let read_operation = operation_of(read);
    let (input, mut unit) = session.into_parts();
    // Hand-move only the read: its source still spells the local's
    // member-produced root, but the establishment stayed inside — the freeze
    // fence re-derives the admission with an empty relocated-root set for the
    // component and refuses because no moved node covered the observed root.
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
fn member_parameter_rebinds_to_a_run_covered_member_produced_root() {
    let session = lowered_session(MEMBER_PRODUCED_VIEW_SOURCE, "member-produced view loop");
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
    // `swap`'s `other` member parameter binds only the subslice root `step`
    // establishes each traversal — member-produced, so the parameter resolves
    // only when the relocation run covers the subslice's declared place.
    let (subslice, lengths) = component
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
        .fold((None, Vec::new()), |(mut subslice, mut lengths), node| {
            match &node.operation {
                AbstractOperation::ByteSequenceSubslice { .. } => subslice = Some(node),
                AbstractOperation::ByteSequenceLength { .. } => lengths.push(node),
                _ => {}
            }
            (subslice, lengths)
        });
    let subslice = subslice.expect("the member subslice exists");
    let (subslice_operation, subslice_place, subslice_source) = match &subslice.operation {
        AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, result.place, *source),
        operation => panic!("the member node is a subslice: {operation:?}"),
    };
    let [_, other_length] = lengths.as_slice() else {
        panic!("`view.len` and `other.len` are the member length observations")
    };
    let (length_operation, length_source) = match &other_length.operation {
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            source,
            ..
        } => (*psi_operation, *source),
        operation => panic!("the member node is a length observation: {operation:?}"),
    };
    assert_ne!(
        length_source, subslice_source,
        "`other.len` observes `swap`'s own parameter, not `step`'s `view`"
    );
    // The observation root is `swap`'s member parameter; every reaching edge
    // binds it to the member-produced subslice root, so the representative is
    // the covered root — not a preheader-visible place.
    assert!(
        !crate::validation::place_observations::place_observation_root_visible(
            function,
            function
                .blocks
                .iter()
                .find(|block| block.id == entry.source)
                .expect("preheader exists"),
            subslice_place,
        ),
        "the member-produced subslice root is not preheader-visible"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let subslice_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == subslice_operation)
        .expect("the member subslice relocates");
    let length_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == length_operation)
        .expect("the member observation relocates behind its covered root's producer");
    assert_eq!(
        length_relocation.node().root_rewrite(),
        Some((length_source, subslice_place)),
        "the moved observation rebinds its member parameter to the covered member-produced root"
    );
    assert!(
        subslice_relocation.destination().node < length_relocation.destination().node,
        "the relocated subslice lands ahead of the observation spelling its root"
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
        .find(|block| block.id == length_relocation.destination().block)
        .expect("destination block exists");
    let moved_length =
        &destination.nodes[usize::try_from(length_relocation.destination().node).unwrap()];
    match &moved_length.operation {
        AbstractOperation::ByteSequenceLength { source, .. } => assert_eq!(
            *source, subslice_place,
            "the relocated observation spells the covered member-produced root"
        ),
        operation => panic!("relocated observation keeps its operation: {operation:?}"),
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}
