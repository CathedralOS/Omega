//! Loop-invariant place observations: place reads over an invariant root,
//! member view parameters resolving across member edges, and member reads
//! of a member-produced root the same run relocates.

use crate::VerifiedPsiOptimizationSession;
use crate::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{
    NodeLocation, ProvenanceDisposition, PsiProvenance, PsiRealizationSite,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::OptimizationUnitValidationError;

use super::{
    find_operation_mut, lowered_session, lowered_session_entry, member_field_reads,
    member_length_reads, member_primitive_local, member_structural_scalar_call, operation_of,
    refresh_coordinates_and_effects, take_operation,
};

/// A `self`-carried countdown whose member observes a field through the
/// function's structural-parameter root: `self.value` lowers to an
/// `IntegerStructuralField` naming `PlaceId(1)` directly — no member
/// structural parameter stands between the read and the root — the component
/// performs no place mutation or custody movement at all, and `step` is the
/// only member, so every traversal that leaves passes through it. The
/// observation relocates into the preheader byte-exact and the scalar
/// computation chained on the observed result relocates behind it through the
/// same run.
const OBSERVED_FIELD_SOURCE: &str = r#"
    data Root { value: u32 in Wrapping }

    machine Root::scan(&mut self, remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(&mut self, pending: u32 [0..=5]) {
            let observed: u32 in Wrapping = self.value + 0;
            transition pending > 0 {
                true -> step(pending - 1)
                _ -> finish(observed)
            }
        }
        state finish(&mut self, r: u32 in Wrapping) {}
    }
"#;

/// Same component shape with a member store: `self.ticks = self.ticks + 1`
/// writes through a root the traversal can reach, so the whole-component
/// custody gate refuses every place observation — the next iteration may
/// observe different storage — while scalar-constant leaves still relocate.
const OBSERVED_FIELD_MUTATED_SOURCE: &str = r#"
    data Root { value: u32 in Wrapping; ticks: u32 in Wrapping; }

    machine Root::scan(&mut self, remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(&mut self, pending: u32 [0..=5]) {
            let observed: u32 in Wrapping = self.value + 0;
            self.ticks = self.ticks + 1;
            transition pending > 0 {
                true -> step(pending - 1)
                _ -> finish(observed)
            }
        }
        state finish(&mut self, r: u32 in Wrapping) {}
    }
"#;

/// The place-custody gate is intact — no member writes any place — but the
/// observation lives in `step`, a member the entry state's `done` arm can
/// bypass. Observing `self.value` performs work a skipped traversal never
/// pays, so the non-speculative gate keeps the read inside even though the
/// observation itself is loop-invariant.
const BYPASSED_OBSERVED_FIELD_SOURCE: &str = r#"
    data Root { value: u32 in Wrapping }

    machine Root::scan(&mut self, scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(&mut self, s: u32 in Wrapping, pending: u32 [0..=5]) {
            let observed: u32 in Wrapping = self.value + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(observed)
            }
        }
        state done(&mut self) {}
        state finish(&mut self, r: u32 in Wrapping) {}
    }
"#;

/// A `view` member structural parameter forwarded unchanged on the back edge:
/// `step` binds `view` from the machine's `entries` root on the entry edge and
/// to itself on the back edge, so the member view parameter resolves
/// transitively to the preheader-visible `entries` root — the place analog of
/// `s`'s scalar resolution in `TRANSITIVE_MEMBER_SOURCE`. `view.len` is an
/// invariant place observation through that member parameter: the relocation
/// rebinds the observed root to `entries` rather than moving byte-exact, and
/// `s + s` relocates behind it through the ordinary scalar path in the same
/// run.
const MEMBER_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], pending: u32 [0..=5]) {
            let length: u64 = view.len;
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> step(s, view, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

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

/// Same `view` member parameter shape, but the back edge forwards a fresh
/// subslice the member itself establishes, so `view` is re-established inside
/// the component on every traversal and stays loop-carried: the observation
/// can rebind to no representative and stays inside. The subslice
/// establishment also ends the component's place-custody preservation, so the
/// custody gate refuses first — the member-produced binding keeps the refusal
/// exact at the resolution level too.
const CARRIED_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], pending: u32 [0..=5]) {
            let length: u64 = view.len;
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 && view.len > 0 {
                true -> step(s, view[1..], pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

#[test]
fn invariant_place_observation_relocates_with_its_chained_computation() {
    let session = lowered_session(OBSERVED_FIELD_SOURCE, "observed field loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
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
    let reads = member_field_reads(function, component);
    let [(member_block, read)] = reads.as_slice() else {
        panic!("one member field observation")
    };
    let member = member_block.id;
    let (read_source, read_field) = match &read.operation {
        AbstractOperation::IntegerStructuralField { source, field, .. } => (*source, *field),
        _ => unreachable!("member_field_reads only yields field observations"),
    };
    assert!(
        function
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == read_source),
        "the observed root is the function's `self` structural parameter"
    );
    assert!(
        crate::validation::place_observations::component_preserves_place_observations(
            function, component
        ),
        "no member mutates or moves custody of any place"
    );
    assert_eq!(
        crate::validation::place_observations::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        ),
        Some(read_source),
        "the field observation passes the shared admission and keeps its own root"
    );
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let read_result = match read.definitions.as_slice() {
        [definition] => definition.value,
        _ => panic!("one observed result"),
    };
    let addition = member_block
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("the observed value feeds a scalar computation");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the computation carries its operation identity"),
    };
    match &addition.operation {
        AbstractOperation::WrappingIntegerAdd { left, .. } => assert_eq!(*left, read_result),
        _ => unreachable!("the add reads the observed result"),
    }
    let expected_parameters = function.structural_parameters.clone();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the place observation is a planned relocation");
    assert!(
        relocation.node().operand_rewrites().is_empty(),
        "the observed root already names a preheader-visible place"
    );
    assert_eq!(relocation.node().result().scalar_value(), Some(read_result));
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    let chained = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("the computation chained on the observation relocates in the same run");
    assert!(
        chained.node().operand_rewrites().is_empty(),
        "the run preserves the observed result's value identity"
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
        AbstractOperation::IntegerStructuralField {
            source,
            field,
            result,
            ..
        } => {
            assert_eq!(
                *source, read_source,
                "the relocated observation keeps its root"
            );
            assert_eq!(*field, read_field);
            assert_eq!(result.value, read_result);
        }
        operation => panic!("relocated observation keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    assert!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .nodes
            .iter()
            .all(|node| !matches!(
                node.operation,
                AbstractOperation::IntegerStructuralField { .. }
            )),
        "the observation exists once, at the destination"
    );
    // Place custody survives the transform: the root stays declared and the
    // function's structural-parameter roster is unchanged.
    assert!(output_function.declared_places.contains(&read_source));
    assert_eq!(output_function.structural_parameters, expected_parameters);

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated observation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn member_store_keeps_place_observations_inside() {
    let session = lowered_session(OBSERVED_FIELD_MUTATED_SOURCE, "mutated field loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
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
        "the member store ends the component's place-custody preservation"
    );
    let reads = member_field_reads(function, component);
    assert_eq!(reads.len(), 2, "the member observes both `self` fields");
    let read_operations = reads
        .iter()
        .map(|(_, node)| match node.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => *operation,
            _ => panic!("the observation carries its operation identity"),
        })
        .collect::<Vec<_>>();
    for (_, read) in &reads {
        assert!(
            crate::validation::place_observations::invariant_place_observation_admission(
                function,
                component,
                read,
                &std::collections::BTreeSet::new(),
            )
            .is_none(),
            "the shared admission refuses the observation"
        );
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty()
            && candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().operand_rewrites().is_empty()),
        "work-free scalar-constant leaves still relocate"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| !read_operations.contains(&relocation.node().psi_operation())),
        "no observation relocates out of a mutating component"
    );
}

#[test]
fn mutated_component_read_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(OBSERVED_FIELD_MUTATED_SOURCE, "mutated field loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
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
    let (member_block, read) = member_field_reads(function, component)[0];
    let member = member_block.id;
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let (input, mut unit) = session.into_parts();
    // Hand-move a field observation out of a component that stores through a
    // reachable root: the relocation fence must refuse it because the
    // seed-derived custody gate fails, not merely because the shape differs.
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
fn bypassed_member_place_observation_stays_inside() {
    let session = lowered_session(BYPASSED_OBSERVED_FIELD_SOURCE, "bypassed read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
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
        "the custody gate is intact; only speculation refuses the observation"
    );
    let (observing_block, read) = member_field_reads(function, component)[0];
    assert!(
        !crate::validation::member_blocks::guaranteed_executed_member_blocks(component)
            .contains(&observing_block.id),
        "the observing member does not dominate every exit"
    );
    assert!(
        crate::validation::invariant_operations::admissible_invariant_place_read(read).is_some(),
        "the observation's own shape is admitted"
    );
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty(),
        "work-free scalar-constant leaves still relocate"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != read_operation),
        "the speculated observation is not a planned relocation"
    );
}

#[test]
fn member_view_parameter_observation_relocates_rebinding_its_root() {
    let session = lowered_session(MEMBER_VIEW_SOURCE, "member view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
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
    let member = component.members[0];
    let member_block = function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    let [view_parameter] = member_block.structural_parameters.as_slice() else {
        panic!("the member carries one view structural parameter")
    };
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    assert_eq!(
        crate::validation::place_observations::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .get(&view_parameter.place),
        Some(&entries_parameter.place),
        "the member view parameter resolves transitively to the machine's `entries` root"
    );
    let reads = member_length_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member length observation")
    };
    assert_eq!(
        crate::validation::invariant_operations::admissible_invariant_place_read(read),
        Some(view_parameter.place),
        "the observation reads through the member view parameter"
    );
    assert_eq!(
        crate::validation::place_observations::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        ),
        Some(entries_parameter.place),
        "the admission rebinds the observed root to the preheader-visible representative"
    );
    assert!(
        crate::validation::place_observations::component_preserves_place_observations(
            function, component
        ),
        "no member mutates or moves custody of any place"
    );
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let read_result = match read.definitions.as_slice() {
        [definition] => definition.value,
        _ => panic!("one observed result"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the member view observation is a planned relocation");
    assert_eq!(
        relocation.node().root_rewrite(),
        Some((view_parameter.place, entries_parameter.place)),
        "the relocation records the member parameter's root rebind"
    );
    assert!(
        relocation.node().operand_rewrites().is_empty(),
        "a place observation rebinds its root, not a scalar operand"
    );
    assert_eq!(relocation.node().result().scalar_value(), Some(read_result));
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    // The scalar member parameter still relocates through its own path in the
    // same run: `s + s` rebinds `s` to `scale`, not through a root rewrite.
    let addition = member_block
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("the member carries the scalar computation");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the computation carries its operation identity"),
    };
    let chained = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("the scalar member-parameter computation relocates in the same run");
    assert_eq!(chained.node().root_rewrite(), None);
    assert_eq!(chained.node().operand_rewrites().len(), 1);
    let view_place = view_parameter.place;
    let entries_place = entries_parameter.place;
    let member_structural_parameters = member_block.structural_parameters.clone();

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
        AbstractOperation::ByteSequenceLength { source, .. } => {
            assert_eq!(
                *source, entries_place,
                "the relocated observation reads the representative root"
            );
        }
        operation => panic!("relocated observation keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    assert!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::ByteSequenceLength { .. })),
        "the observation exists once, at the destination"
    );
    // Place custody survives the transform: both roots stay declared and the
    // member's view parameter roster is unchanged — the moved observation
    // reads the representative, it does not rebind the parameter itself.
    assert!(output_function.declared_places.contains(&view_place));
    assert!(output_function.declared_places.contains(&entries_place));
    assert_eq!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .structural_parameters,
        member_structural_parameters,
    );

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated observation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

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

#[test]
fn member_produced_view_parameter_stays_loop_carried() {
    let session = lowered_session(CARRIED_VIEW_SOURCE, "carried view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let observing = function
        .blocks
        .iter()
        .find(|block| {
            block
                .nodes
                .iter()
                .any(|node| matches!(node.operation, AbstractOperation::ByteSequenceLength { .. }))
        })
        .expect("one member observes the view length");
    let [view_parameter] = observing.structural_parameters.as_slice() else {
        panic!("the observing member carries one view structural parameter")
    };
    assert!(
        !crate::validation::place_observations::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .contains_key(&view_parameter.place),
        "a member-produced back-edge binding keeps the member view parameter loop-carried"
    );
    // The member's own subslice establishment no longer closes the custody
    // gate: it reads the carried root's extent without mutating it and its
    // fresh view is a member-produced root that can never anchor another
    // parameter's invariant representative. The refusal for this component
    // lives entirely in the root resolution — `view` is bound to a
    // member-produced place on the back edge, so it stays loop-carried and
    // every observation through it refuses below.
    assert!(
        crate::validation::place_observations::component_preserves_place_observations(
            function, component
        ),
        "establishing a fresh view mutates no established place"
    );
    let reads = member_length_reads(function, component);
    let read_operations: Vec<_> = reads
        .iter()
        .map(|(_, read)| {
            assert!(
                crate::validation::place_observations::invariant_place_observation_admission(
                    function,
                    component,
                    read,
                    &std::collections::BTreeSet::new(),
                )
                .is_none(),
                "the shared admission refuses each observation on the carried root"
            );
            match read.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => *operation,
                _ => panic!("each observation carries its operation identity"),
            }
        })
        .collect();
    assert!(
        !read_operations.is_empty(),
        "the member observes the carried view"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty(),
        "scalar work still relocates out of the same component"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| { !read_operations.contains(&relocation.node().psi_operation()) }),
        "the loop-carried view observations are not planned relocations"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().root_rewrite().is_none()),
        "no relocation carries a root rewrite"
    );
    // The scalar member parameter is genuinely invariant — every reaching edge
    // resolves `s` back to `scale` — so `s + s` still relocates through the
    // ordinary scalar path while the carried view stays inside.
    let addition = observing
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("the observing member carries the scalar computation");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the computation carries its operation identity"),
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .any(|relocation| relocation.node().psi_operation() == addition_operation),
        "the invariant scalar computation still relocates"
    );
}

#[test]
fn carried_member_view_observation_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(CARRIED_VIEW_SOURCE, "carried view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
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
    let (observing_block, read) = member_length_reads(function, component)[0];
    let member = observing_block.id;
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let (input, mut unit) = session.into_parts();
    // Hand-move a view observation whose member parameter never resolves: the
    // relocation fence must refuse it because no representative exists, not
    // merely because the shape differs.
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
fn forged_view_root_rewrite_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_VIEW_SOURCE, "member view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let machine = component.id.machine;
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().root_rewrite().is_some())
        .expect("the member view observation carries a root rewrite");
    let member = relocation.node().location().block;
    let (parameter, _) = relocation
        .node()
        .root_rewrite()
        .expect("root rewrite exists");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound root back to the member view parameter must fail the
    // seed-derived resolution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, relocation.node().psi_operation());
    if let AbstractOperation::ByteSequenceLength { source, .. } = &mut forged.operation {
        *source = parameter;
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

/// Same component shape, but the borrowed local's initializer is the
/// loop-carried countdown: the establishment cannot leave, so the run never
/// covers `scratch`'s root — the member read, the borrowing call, and the
/// chained computation all stay inside even though the call's scalar
/// argument is invariant.
const CARRIED_LOCAL_READ_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = remaining;
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
fn member_read_stays_when_its_producer_cannot_relocate() {
    let session =
        lowered_session_entry(CARRIED_LOCAL_READ_SOURCE, "carried local read loop", "scan");
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
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    let read_operation = operation_of(read);
    assert!(
        crate::validation::place_observations::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        )
        .is_none(),
        "the member-produced root is not run-covered while its producer stays"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // The establishment's initializer is the loop-carried countdown, so
        // the producer never enters the run — and with the root uncovered,
        // the read and the borrowing call have no landing either.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && relocation.node().psi_operation() != call_operation
                        && relocation.node().psi_operation() != read_operation
                ),
            "a staying producer keeps the local, the call, and the read inside"
        );
    }
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
