use super::*;

fn location(number: u64) -> DescriptorLocation {
    DescriptorLocation::whole(PlaceId::new(number).unwrap())
}

#[test]
fn completed_position_maps_share_ownership_preserve_snapshots_and_failure_fences() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowEdge, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, ScalarType};
    use terminal_psi::TerminalIdentityIndex;

    let id = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let positions = [(block, 0), (block, 1), (block, 2)];
    let parameter = location(1);
    let replacement_parameter = location(2);
    let scalar = StructuralTypeId::new(1).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let collection = StructuralTypeId::new(3).unwrap();
    let element = StructuralTypeId::new(4).unwrap();
    let referent = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: scalar,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )),
        scalar_restriction: None,
    };
    let reference = StructuralTypeDeclaration {
        id: carrier,
        identity: TerminalIdentityIndex(2),
        shape: StructuralTypeShape::Reference {
            referent: collection,
            access: StructuralAccess::SharedBorrow,
        },
        ..referent.clone()
    };
    let pointer = StructuralTypeDeclaration {
        id: element,
        identity: TerminalIdentityIndex(4),
        shape: StructuralTypeShape::Reference {
            referent: scalar,
            access: StructuralAccess::SharedBorrow,
        },
        ..reference.clone()
    };
    let array = StructuralTypeDeclaration {
        id: collection,
        identity: TerminalIdentityIndex(3),
        shape: StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(1),
        },
        ..reference.clone()
    };
    let rows = StructuralTypeId::new(5).unwrap();
    let holder = location(3);
    let nested = StructuralTypeDeclaration {
        id: rows,
        identity: TerminalIdentityIndex(5),
        shape: StructuralTypeShape::FixedArray {
            element: collection,
            length: terminal_psi::ArrayExtent::Literal(1),
        },
        ..reference.clone()
    };
    let types = vec![referent, reference, array, pointer, nested];
    let mut flow = BTreeMap::new();
    for (index, position) in positions.iter().copied().enumerate() {
        let mut step = DescriptorFlowStep::default();
        if let Some(&target) = positions.get(index + 1) {
            step.successors.push(DescriptorFlowEdge {
                effects: Vec::new(),
                edge: None,
                definitions: Vec::new(),
                target,
                parameters: Vec::new(),
                bindings: Vec::new(),
            });
        }
        if index == 0 {
            step.definitions.push(parameter.clone());
        }
        flow.insert(position, step);
    }
    let machine = |id| DescriptorMachine {
        id,
        returns: None,
        places: vec![
            DescriptorPlace {
                producer: None,
                place: parameter.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::Owned,
                    structural_type: carrier,
                },
            },
            DescriptorPlace {
                producer: None,
                place: replacement_parameter.root,
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::Owned,
                    structural_type: carrier,
                },
            },
            DescriptorPlace {
                producer: None,
                place: holder.root,
                definition: DescriptorDefinition::Parameter {
                    position: 2,
                    access: StructuralAccess::Owned,
                    structural_type: rows,
                },
            },
        ],
        place_types: BTreeMap::from([
            (parameter.root, carrier),
            (replacement_parameter.root, carrier),
            (holder.root, rows),
        ]),
        entry: positions[0],
        flow: flow.clone(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: None,
        unsigned_index_literals: Vec::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        private_observation_inputs: Default::default(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: vec![positions[2]],
    };
    let mut graph = vec![machine(id)];
    let mut replay = Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    let completed = replay.states(id, 0).unwrap();
    assert_eq!(completed.keys().copied().collect::<Vec<_>>(), positions);
    let original = Some(BTreeSet::from([Origin::Incoming(parameter.clone())]));
    assert_eq!(
        completed[&positions[0]].slots.get(&parameter),
        Some(&original)
    );
    assert_eq!(completed[&positions[1]].slots.get(&parameter), Some(&None));
    assert!(Arc::ptr_eq(
        &completed,
        &replay.snapshots[&id].ready().unwrap().before
    ));
    assert!(Arc::ptr_eq(&completed, &replay.states(id, 0).unwrap()));
    assert!(Arc::ptr_eq(&completed, &replay.states(id, 128).unwrap()));
    replay.prepare_states(id).unwrap();
    let mut edited = completed[&positions[0]].clone();
    edited.slots_mut().insert(parameter.clone(), None);
    assert_eq!(
        completed[&positions[0]].slots.get(&parameter),
        Some(&original)
    );
    assert_eq!(edited.slots.get(&parameter), Some(&None));

    let mut ancestor = parameter.clone();
    ancestor.path.push(DescriptorPathSegment::Referent);
    let mut leaf = ancestor.clone();
    leaf.path.push(DescriptorPathSegment::FixedIndex(0));
    let query = DescriptorContentsQuery::from_location(&leaf);
    let mut current = completed[&positions[0]].clone();
    let observed = replay.read(&graph[0], &current, &leaf);
    assert!(observed.is_some());
    assert_eq!(replay.read_contents(&graph[0], &current, &query), observed);
    current.slots_mut().insert(ancestor, None);
    assert!(replay.read(&graph[0], &current, &leaf).is_none());
    assert!(replay.read_contents(&graph[0], &current, &query).is_none());
    let mut replacement_leaf = leaf.clone();
    replacement_leaf.root = replacement_parameter.root;
    let replacement = replay.read(&graph[0], &current, &replacement_leaf);
    assert!(replacement.is_some());
    assert_ne!(replacement, observed);
    current
        .slots_mut()
        .insert(leaf.clone(), replacement.clone());
    assert_eq!(replay.read(&graph[0], &current, &leaf), replacement);
    assert_eq!(
        replay.read_contents(&graph[0], &current, &query),
        replacement
    );

    let mut vacant = completed[&positions[0]].clone();
    vacant.slots_mut().insert(leaf.clone(), None);
    assert!(vacant.contents_path_invalidated(&query));
    assert!(!replay.contents_address_hidden(&graph[0], &vacant, &query));
    assert!(
        replay
            .contents_targets(&graph[0], &vacant, &query)
            .is_some_and(|targets| !targets.is_empty())
    );
    assert!(replay.read_contents(&graph[0], &vacant, &query).is_none());
    let mut ancestor_query = query.clone();
    ancestor_query.steps.pop();
    let mut below = ancestor_query.clone();
    below.steps.push(DescriptorContentsStep::Address(
        DescriptorPathSegment::FixedIndex(0),
    ));
    let mut hidden = completed[&positions[0]].clone();
    hidden.slots_mut().insert(
        DescriptorLocation {
            root: ancestor_query.root,
            path: ancestor_query
                .steps
                .iter()
                .map(|step| match step {
                    DescriptorContentsStep::Address(segment) => *segment,
                    DescriptorContentsStep::Element { .. } => unreachable!(),
                })
                .collect(),
        },
        None,
    );
    assert!(hidden.contents_path_invalidated(&below));
    assert!(replay.contents_address_hidden(&graph[0], &hidden, &below));
    assert!(
        replay
            .contents_targets(&graph[0], &hidden, &below)
            .is_none()
    );

    // A moved-out aggregate keeps its parts in place: the row is
    // unknown, yet a store still finds the cell inside it, while a
    // read of that cell stays unknown.
    let mut row = holder.clone();
    row.path.push(DescriptorPathSegment::FixedIndex(0));
    let mut cell = row.clone();
    cell.path.push(DescriptorPathSegment::FixedIndex(0));
    let cell_query = DescriptorContentsQuery::from_location(&cell);
    let mut moved = completed[&positions[0]].clone();
    moved.slots_mut().insert(row, None);
    moved.slots_mut().insert(cell.clone(), None);
    assert!(moved.contents_path_invalidated(&cell_query));
    assert!(!replay.contents_address_hidden(&graph[0], &moved, &cell_query));
    assert_eq!(
        replay.contents_targets(&graph[0], &moved, &cell_query),
        Some(BTreeSet::from([DescriptorPayloadTarget::current(cell)]))
    );
    assert!(
        replay
            .read_contents(&graph[0], &moved, &cell_query)
            .is_none()
    );
    // Storage identity survives a loss of readable contents. Repeated
    // queries must agree with direct replay at each completed cut.
    fn send_and_sync<T: Send + Sync>() {}
    send_and_sync::<DescriptorAnalysis<'_>>();
    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
    let original_backing = analysis.location(id, positions[0], &leaf);
    assert!(original_backing.is_some());
    for position in positions[..2].iter().copied() {
        assert_eq!(
            analysis.location(id, position, &leaf),
            replay.canonical_storage(&graph[0], &completed[&position], &leaf, None)
        );
    }
    assert_eq!(analysis.location(id, positions[1], &leaf), original_backing);
    assert_eq!(analysis.location(id, positions[0], &leaf), original_backing);
    assert_eq!(analysis.location(id, positions[1], &leaf), original_backing);
    let foreign = location(u64::MAX);
    assert_eq!(analysis.location(id, positions[0], &foreign), None);
    assert_eq!(analysis.location(id, positions[0], &foreign), None);
    drop(analysis);

    drop(replay);

    // Replacing the reference really changes the later address. A
    // cached query at either cut must not overwrite the other cut.
    graph[0]
        .flow
        .get_mut(&positions[0])
        .unwrap()
        .derivations
        .push((replacement_parameter.clone(), parameter.clone()));
    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
    assert_eq!(
        analysis.location(id, positions[1], &leaf),
        Some(replacement_leaf.clone())
    );
    assert_eq!(analysis.location(id, positions[0], &leaf), original_backing);
    assert_eq!(
        analysis.location(id, positions[1], &leaf),
        Some(replacement_leaf)
    );
    drop(analysis);
    graph[0]
        .flow
        .get_mut(&positions[0])
        .unwrap()
        .derivations
        .clear();

    let mut limited =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    assert!(limited.states(id, 128).is_none());
    let limited_error = limited.prepare_states_checked(id).unwrap_err();
    assert_eq!(limited_error.machine(), id);
    assert_eq!(
        limited_error.detail(),
        Some("descriptor state preparation reached its depth bound")
    );
    assert!(limited.states(id, 0).is_none());
    assert_eq!(limited.prepare_states_checked(id), Err(limited_error));
    drop(limited);
    graph[0]
        .flow
        .insert((block, 99), DescriptorFlowStep::default());
    let mut unavailable =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    assert!(unavailable.states(id, 0).is_none());
    let graph_error = unavailable.prepare_states_checked(id).unwrap_err();
    assert_eq!(graph_error.machine(), id);
    assert_eq!(
        graph_error.detail(),
        Some("descriptor case graph or invocation owners are incomplete")
    );
    // The same IDs in another graph have their own preparation and
    // cannot borrow a successful query from the earlier analysis.
    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
    assert_eq!(analysis.location(id, positions[0], &leaf), None);
    assert_eq!(analysis.location(id, positions[0], &leaf), None);
    assert_eq!(completed.len(), 3);
    assert_eq!(
        completed[&positions[0]].slots.get(&parameter),
        Some(&original)
    );
    drop(analysis);
    drop(unavailable);
    graph[0].flow.remove(&(block, 99));
    graph[0].returns = Some(vec![(positions[2], replacement_parameter.clone())]);
    let caller = MachineId::new(2).unwrap();
    let mut outer = machine(caller);
    let sources = vec![
        parameter.clone(),
        replacement_parameter.clone(),
        holder.clone(),
    ];
    let argument_contents = sources
        .iter()
        .map(|argument| Some(DescriptorContentsQuery::from_location(argument)))
        .collect();
    outer.calls.insert(
        positions[0],
        Some(vec![DescriptorInvocation {
            scalar_arguments: Vec::new(),
            owner: DescriptorInvocationOwner::Operation(
                semantic_vocabulary::OperationId::new(1).unwrap(),
            ),
            target: DescriptorInvocationTarget::Machine(id),
            arguments: sources.into_iter().map(Some).collect(),
            argument_contents,
            result: None,
        }]),
    );
    graph.push(outer);
    let mut complete = DescriptorAnalysis::with_reference_slots(&types, &graph);
    assert!(complete.flow(caller).is_ok());
    drop(complete);

    graph[0]
        .flow
        .get_mut(&positions[0])
        .unwrap()
        .consumed
        .push(replacement_parameter.clone());
    let inner_error = DescriptorGraphError::IncompleteMachine {
        machine: id,
        position: Some(positions[0]),
        successor: None,
        detail: Some("descriptor storage transfer cannot be reconstructed"),
    };
    let mut nested = DescriptorAnalysis::with_reference_slots(&types, &graph);
    // Returning a reference requires the callee's complete summary.
    // Its consumed parameter now lacks the retirement definition;
    // preserve that inner failure when the outer call asks again.
    assert_eq!(nested.flow(caller).err().unwrap(), inner_error);
    assert_eq!(nested.flow(caller).err().unwrap(), inner_error);
    let mut preparing =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    preparing
        .snapshots
        .insert(caller, PreparedStateCache::Preparing);
    assert!(preparing.states(caller, 0).is_none());
    let error = preparing.prepare_states_checked(caller).unwrap_err();
    assert_eq!(error.machine(), caller);
    assert_eq!(
        error.detail(),
        Some("descriptor state preparation is recursively in progress")
    );
}

#[test]
fn replacement_preserves_both_maps_in_pre_write_snapshot() {
    let slot = location(1);
    let query = DescriptorContentsQuery {
        root: slot.root,
        steps: Vec::new(),
    };
    let old = Some(BTreeSet::from([Origin::Storage(location(2))]));
    let new = Some(BTreeSet::from([Origin::Storage(location(3))]));
    let mut before = State::default();
    before.slots_mut().insert(slot.clone(), old.clone());
    before.contents_mut().insert(query.clone(), old.clone());
    let mut after = before.clone();
    Replay::install_contents(
        &mut after,
        BTreeMap::from([
            (DescriptorPayloadTarget::current(slot.clone()), new.clone()),
            (
                DescriptorPayloadTarget::Incoming(query.clone()),
                new.clone(),
            ),
        ]),
    );
    after.contents_invalidated = true;
    assert_eq!(before.slots.get(&slot), Some(&old));
    assert_eq!(before.contents.get(&query), Some(&old));
    assert!(!before.contents_invalidated);
    assert_eq!(after.slots.get(&slot), Some(&new));
    assert_eq!(after.contents.get(&query), Some(&new));
}

#[test]
fn equal_input_states_still_materialize_incoming_contents_at_join() {
    let query = DescriptorContentsQuery {
        root: location(1).root,
        steps: Vec::new(),
    };
    let original = BTreeSet::from([Origin::Storage(location(2))]);
    let mut before = State::default();
    before
        .contents_mut()
        .insert(query.clone(), Some(original.clone()));
    let mut joined = before.clone();
    assert!(merge_state(&mut joined, &before));
    let mut expected = original.clone();
    expected.insert(Origin::IncomingContents(
        query.clone(),
        Projection::new(&[]),
    ));
    assert_eq!(joined.contents.get(&query), Some(&Some(expected)));
    assert_eq!(before.contents.get(&query), Some(&Some(original)));
    assert!(!merge_state(&mut joined, &before));
}
#[test]
fn ordered_slot_join_preserves_absence_unknown_and_captured_origins() {
    let variants = [
        None,
        Some(None),
        Some(Some(BTreeSet::new())),
        Some(Some(BTreeSet::from([Origin::Storage(location(8))]))),
        Some(Some(BTreeSet::from([Origin::Storage(location(9))]))),
    ];
    for left in &variants {
        for right in &variants {
            for (left_invalid, right_invalid) in
                [(false, false), (false, true), (true, false), (true, true)]
            {
                let mut before = State::default();
                let mut incoming = State::default();
                let mut descendant = location(3);
                descendant.path.push(DescriptorPathSegment::FixedIndex(2));
                for (key, value) in [
                    (location(1), left),
                    (location(3), right),
                    (descendant.clone(), left),
                ] {
                    if let Some(value) = value {
                        before.slots_mut().insert(key, value.clone());
                    }
                }
                for (key, value) in [
                    (location(2), right),
                    (location(3), left),
                    (descendant, right),
                    (location(5), left),
                ] {
                    if let Some(value) = value {
                        incoming.slots_mut().insert(key, value.clone());
                    }
                }
                before.contents_invalidated = left_invalid;
                incoming.contents_invalidated = right_invalid;
                let original_slots = (*before.slots).clone();
                let snapshot = before.clone();
                let mut expected = before.clone();
                expected.contents_invalidated |= right_invalid;
                let keys = before
                    .slots
                    .keys()
                    .chain(incoming.slots.keys())
                    .cloned()
                    .collect::<BTreeSet<_>>();
                for key in keys {
                    let value = match (before.slots.get(&key), incoming.slots.get(&key)) {
                        (Some(Some(left)), Some(Some(right))) => {
                            Some(left.union(right).cloned().collect())
                        }
                        _ => None,
                    };
                    expected.slots_mut().insert(key, value);
                }
                let expected_changed = expected != snapshot;
                let changed = merge_state(&mut before, &incoming);
                assert_eq!(changed, expected_changed);
                assert_eq!(before.slots, expected.slots);
                assert_eq!(before.contents_invalidated, expected.contents_invalidated);
                assert!(before.contents == expected.contents);
                assert_eq!(*snapshot.slots, original_slots);
            }
        }
    }
}
#[test]
fn join_slot_updates_release_only_the_scratch_snapshot_before_writing() {
    let mut existing = State::default();
    for index in 1..=64 {
        existing.slots_mut().insert(
            location(index),
            Some(BTreeSet::from([Origin::Storage(location(100 + index))])),
        );
    }
    let mut incoming = State {
        slots: Arc::new((*existing.slots).clone()),
        ..State::default()
    };
    incoming.slots_mut().insert(
        location(64),
        Some(BTreeSet::from([Origin::Storage(location(200))])),
    );
    assert_eq!(Arc::strong_count(&existing.slots), 1);
    assert_eq!(Arc::weak_count(&existing.slots), 0);
    let unique_map = Arc::as_ptr(&existing.slots);
    assert!(merge_state(&mut existing, &incoming));
    assert_eq!(Arc::as_ptr(&existing.slots), unique_map);
    assert_eq!(
        existing.slots.get(&location(64)),
        Some(&Some(BTreeSet::from([
            Origin::Storage(location(164)),
            Origin::Storage(location(200)),
        ]))),
    );
    let retained = existing.clone();
    incoming.slots = Arc::new((*existing.slots).clone());
    assert!(!merge_state(&mut existing, &incoming));
    assert!(Arc::ptr_eq(&existing.slots, &retained.slots));
    let original_slots = (*retained.slots).clone();
    incoming.slots_mut().insert(
        location(64),
        Some(BTreeSet::from([Origin::Storage(location(201))])),
    );
    assert!(merge_state(&mut existing, &incoming));
    assert!(!Arc::ptr_eq(&existing.slots, &retained.slots));
    assert_eq!(*retained.slots, original_slots);
    assert_eq!(
        existing.slots.get(&location(1)),
        retained.slots.get(&location(1))
    );
    assert_eq!(
        existing
            .slots
            .get(&location(64))
            .unwrap()
            .as_ref()
            .unwrap()
            .len(),
        3
    );
}

fn scalar_type(id: u64) -> StructuralTypeDeclaration {
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
    use terminal_psi::TerminalIdentityIndex;
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: StructuralTypeId::new(id).unwrap(),
        identity: TerminalIdentityIndex(id as u32),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )),
        scalar_restriction: None,
    }
}

fn reference_type(
    id: u64,
    referent: StructuralTypeId,
    access: StructuralAccess,
) -> StructuralTypeDeclaration {
    let scalar = scalar_type(id);
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(id as u32),
        shape: StructuralTypeShape::Reference { referent, access },
        ..scalar
    }
}

fn bare_machine(
    id: MachineId,
    places: Vec<DescriptorPlace>,
    place_types: BTreeMap<PlaceId, StructuralTypeId>,
    storage_derivations: BTreeSet<(
        DescriptorPosition,
        DescriptorLocation,
        DescriptorLocation,
    )>,
) -> DescriptorMachine {
    let entry = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
    DescriptorMachine {
        id,
        returns: None,
        places,
        place_types,
        entry,
        flow: BTreeMap::new(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: None,
        unsigned_index_literals: Vec::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        private_observation_inputs: Default::default(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations,
        calls: BTreeMap::new(),
        normal_exits: vec![entry],
    }
}

#[test]
fn replay_joins_diamond_inputs_once_and_keeps_loop_effects() {
    use crate::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorFlowStep};

    let identity = MachineId::new(1).unwrap();
    let mut machine = bare_machine(identity, Vec::new(), BTreeMap::new(), BTreeSet::new());
    let block = machine.entry.0;
    let step = |targets: &[u32]| DescriptorFlowStep {
        successors: targets
            .iter()
            .map(|&index| DescriptorFlowEdge {
                target: (block, index),
                edge: None,
                effects: Vec::new(),
                definitions: Vec::new(),
                parameters: Vec::new(),
                bindings: Vec::new(),
            })
            .collect(),
        ..Default::default()
    };
    // One arm invalidates contents, the other preserves them. Both feed a
    // long straight-line suffix, which should see the joined state once.
    const EXIT: u32 = 80;
    machine.normal_exits = vec![(block, EXIT)];
    machine.unresolved_contents_writes.insert((block, 1));
    machine.flow.insert((block, 1), step(&[3]));
    machine.flow.insert((block, 2), step(&[3]));
    for point in 3..EXIT {
        machine.flow.insert((block, point), step(&[point + 1]));
    }
    for arms in [[1, 2], [2, 1]] {
        machine.flow.insert(machine.entry, step(&arms));
        for backedge in [false, true] {
            machine
                .flow
                .insert((block, EXIT), if backedge { step(&[2]) } else { step(&[]) });
            let mut replay = Replay::new(&[], std::slice::from_ref(&machine));
            let states = replay.states(identity, 0).expect("complete control flow");
            assert_eq!(states.len(), machine.flow.len());
            for (&(_, point), state) in states.iter() {
                assert_eq!(
                    state.contents_invalidated,
                    point >= 3 || (backedge && point == 2)
                );
            }
            let prepared = replay.snapshots[&identity].ready().unwrap();
            if !backedge {
                assert_eq!(prepared.visits, states.len());
            }
            assert!(prepared.normal_exits[&(block, EXIT)].contents_invalidated);
        }
    }
}

#[test]
fn an_unresolved_write_only_invalidates_returns_it_can_reach() {
    use crate::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorFlowStep};

    let identity = MachineId::new(1).unwrap();
    let mut machine = bare_machine(identity, Vec::new(), BTreeMap::new(), BTreeSet::new());
    let entry = machine.entry;
    let write = (entry.0, 1);
    let normal = (entry.0, 2);
    let exits_process = (entry.0, 3);
    let step = |targets: &[DescriptorPosition]| DescriptorFlowStep {
        successors: targets
            .iter()
            .map(|&target| DescriptorFlowEdge {
                target,
                edge: None,
                effects: Vec::new(),
                definitions: Vec::new(),
                parameters: Vec::new(),
                bindings: Vec::new(),
            })
            .collect(),
        ..Default::default()
    };
    machine.normal_exits.push(normal);
    machine.flow = BTreeMap::from([
        (entry, step(&[normal, write])),
        (write, step(&[exits_process])),
        (normal, step(&[])),
        (exits_process, step(&[])),
    ]);
    // An unknown write at a normal exit counts, including its cleanup.
    for (site, invalidates_return) in [(write, false), (normal, true), (entry, true)] {
        machine.unresolved_contents_writes = BTreeSet::from([site]);
        let mut replay = Replay::new(&[], std::slice::from_ref(&machine));
        let summary = replay.summary(identity, 0).expect("valid descriptor graph");
        assert_eq!(summary.invalidates_contents, invalidates_return);
    }
}

#[test]
fn returned_storage_borrows_keep_the_resident_argument_after_replacement() {
    let scalar = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let mut array_declaration = scalar_type(2);
    array_declaration.shape = StructuralTypeShape::FixedArray {
        element: scalar,
        length: terminal_psi::ArrayExtent::Literal(2),
    };
    let types = [
        scalar_type(1),
        array_declaration,
        reference_type(3, array, StructuralAccess::SharedBorrow),
    ];
    let resident = location(1);
    let replacement = location(2);
    let descriptor = location(3);
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: resident.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: array,
                },
            },
            DescriptorPlace {
                producer: None,
                place: replacement.root,
                definition: DescriptorDefinition::Local,
            },
            DescriptorPlace {
                producer: None,
                place: descriptor.root,
                definition: DescriptorDefinition::Local,
            },
        ],
        BTreeMap::from([
            (resident.root, array),
            (replacement.root, array),
            (descriptor.root, reference),
        ]),
        BTreeSet::new(),
    );
    let replay = Replay::with_slot_kind(
        &types,
        std::slice::from_ref(&machine),
        DescriptorSlotKind::BorrowedStorage,
    );
    let mut state = replay.initial(&machine).unwrap();
    state.slots_mut().insert(
        resident.clone(),
        Some(BTreeSet::from([Origin::Storage(replacement.clone())])),
    );
    let arguments = [Some(resident.clone())];
    let contents = [Some(DescriptorContentsQuery::from_location(&resident))];
    let origin = DescriptorStoredOrigin::Parameter {
        position: 0,
        projection: Projection::new(&[StructuralPathSegment::FixedIndex(1)]),
    };
    let mut expected = resident.clone();
    expected.path.push(DescriptorPathSegment::FixedIndex(1));
    assert_eq!(
        replay.substitute_actuals(
            &machine,
            &state,
            DescriptorActualInputs {
                scalar_arguments: &[],
                arguments: &arguments,
                argument_contents: &contents,
            },
            &origin,
        ),
        Some(BTreeSet::from([Origin::Storage(expected)])),
    );

    // A stored reference still follows its current pointer value.
    // Changing it must not pin the result to the descriptor's home.
    let arguments = [Some(descriptor.clone())];
    let contents = [Some(DescriptorContentsQuery::from_location(&descriptor))];
    let origin = DescriptorStoredOrigin::Parameter {
        position: 0,
        projection: Projection::new(&[]),
    };
    for target in [resident, replacement] {
        state.slots_mut().insert(
            descriptor.clone(),
            Some(BTreeSet::from([Origin::Storage(target.clone())])),
        );
        assert_eq!(
            replay.substitute_actuals(
                &machine,
                &state,
                DescriptorActualInputs {
                    scalar_arguments: &[],
                    arguments: &arguments,
                    argument_contents: &contents,
                },
                &origin,
            ),
            Some(BTreeSet::from([Origin::Storage(target)])),
        );
    }
}

#[test]
fn absent_normal_body_observations_preserve_continuations_and_forget_results() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowEdge, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, OperationId, StructuralCaseId};
    use terminal_psi::{StructuralCaseDeclaration, TerminalIdentityIndex};
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let block = BlockId::new(1).unwrap();
    let at = |index| (block, index);
    let input = location(1);
    let result = location(2);
    let scalar = scalar_type(1);
    let reference = reference_type(2, scalar.id, StructuralAccess::SharedBorrow);
    let tagged = StructuralTypeDeclaration {
        shape: StructuralTypeShape::Sum {
            cases: vec![StructuralCaseDeclaration {
                declaration_identity: Default::default(),
                id: StructuralCaseId::new(1).unwrap(),
                identity: TerminalIdentityIndex(3),
                fields: Vec::new(),
            }],
        },
        ..scalar_type(3)
    };
    let types = vec![scalar, reference, tagged];
    // First the captured scalar-only tagged result; then a result
    // place reused across invocations and a mutable stored reference.
    for result_type in [types[2].id, types[1].id] {
        let mutable_slot = result_type == types[1].id;
        let mut body_places = vec![DescriptorPlace {
            producer: None,
            place: result.root,
            definition: DescriptorDefinition::Local,
        }];
        let mut body_types = BTreeMap::from([(result.root, result_type)]);
        if mutable_slot {
            body_places.insert(
                0,
                DescriptorPlace {
                    producer: None,
                    place: input.root,
                    definition: DescriptorDefinition::Parameter {
                        position: 0,
                        access: StructuralAccess::MutableBorrow,
                        structural_type: types[1].id,
                    },
                },
            );
            body_types.insert(input.root, types[1].id);
        }
        let mut body = bare_machine(callee, body_places, body_types, BTreeSet::new());
        body.returns = Some(Vec::new());
        body.normal_exits.clear();
        body.flow.insert(at(0), DescriptorFlowStep::default());
        let mut outer = bare_machine(
            caller,
            vec![
                DescriptorPlace {
                    producer: None,
                    place: input.root,
                    definition: DescriptorDefinition::Parameter {
                        position: 0,
                        access: StructuralAccess::Owned,
                        structural_type: types[1].id,
                    },
                },
                DescriptorPlace {
                    producer: Some(at(0)),
                    place: result.root,
                    definition: DescriptorDefinition::Local,
                },
            ],
            BTreeMap::from([(input.root, types[1].id), (result.root, result_type)]),
            BTreeSet::new(),
        );
        let edge = |target| DescriptorFlowEdge {
            effects: Vec::new(),
            edge: None,
            definitions: Vec::new(),
            target,
            parameters: Vec::new(),
            bindings: Vec::new(),
        };
        outer.flow = BTreeMap::from([
            (
                at(0),
                DescriptorFlowStep {
                    definitions: vec![result.clone()],
                    derivations: if mutable_slot {
                        vec![(input.clone(), result.clone())]
                    } else {
                        Vec::new()
                    },
                    successors: vec![edge(at(1))],
                    ..Default::default()
                },
            ),
            (
                at(1),
                DescriptorFlowStep {
                    definitions: vec![result.clone()],
                    successors: vec![edge(at(2))],
                    ..Default::default()
                },
            ),
            (at(2), DescriptorFlowStep::default()),
        ]);
        let exit = if mutable_slot {
            outer.flow.get_mut(&at(2)).unwrap().successors = vec![edge(at(1)), edge(at(3))];
            outer.flow.insert(at(3), DescriptorFlowStep::default());
            at(3)
        } else {
            at(2)
        };
        outer.normal_exits = vec![exit];
        outer.returns = Some(vec![(exit, result.clone())]);
        let actuals = if mutable_slot {
            vec![Some(input.clone())]
        } else {
            Vec::new()
        };
        let contents = if mutable_slot {
            vec![Some(DescriptorContentsQuery::from_location(&input))]
        } else {
            Vec::new()
        };
        outer
            .insert_call(
                at(1),
                DescriptorInvocation {
                    scalar_arguments: Vec::new(),
                    owner: DescriptorInvocationOwner::Operation(
                        OperationId::new(1).unwrap(),
                    ),
                    target: DescriptorInvocationTarget::Machine(callee),
                    arguments: actuals.clone(),
                    argument_contents: contents.clone(),
                    result: Some(result.clone()),
                },
            )
            .unwrap();
        let mut graph = vec![outer, body];
        let mut replay =
            Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
        let states = replay.states(caller, 0).unwrap();
        assert!(
            states.contains_key(&at(2)),
            "the interface-visible continuation remains"
        );
        assert_eq!(states[&at(2)].slots.get(&result), Some(&None));
        if mutable_slot {
            assert!(states[&at(0)].slots.get(&input).unwrap().is_some());
            assert_eq!(states[&at(2)].slots.get(&input), Some(&None));
            assert_eq!(states[&exit].slots.get(&result), Some(&None));
        }
        assert!(replay.summary(callee, 0).unwrap().result.is_unobserved());
        let mut public = DescriptorInvocation {
            scalar_arguments: Vec::new(),
            owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
            target: DescriptorInvocationTarget::Public {
                effects_machine: Some(callee),
                realizations: Vec::new(),
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                results: if mutable_slot {
                    vec![DescriptorResultSlot {
                        path: Vec::new(),
                        origins: vec![DescriptorStoredOrigin::IncomingSlot {
                            position: 0,
                            path: Vec::new(),
                        }],
                    }]
                } else {
                    Vec::new()
                },
                contents: Vec::new(),
                invalidates_contents: false,
            },
            arguments: actuals,
            argument_contents: contents,
            result: Some(result.clone()),
        };
        let public_summary = replay.invocation_summary(&public, 0).unwrap();
        assert_eq!(
            public_summary.result.slots().unwrap().len(),
            usize::from(mutable_slot)
        );
        assert!(!public_summary.result.is_unobserved());
        assert!(matches!(
            public_summary.effects,
            EffectObservations::Unobserved(_)
        ));
        let DescriptorInvocationTarget::Public {
            effects_machine,
            realizations,
            ..
        } = &mut public.target
        else {
            unreachable!()
        };
        *effects_machine = None;
        *realizations = vec![callee];
        let joined = replay.invocation_summary(&public, 0).unwrap();
        assert_eq!(
            joined.result.slots().unwrap().len(),
            usize::from(mutable_slot)
        );
        assert!(!joined.result.is_unobserved());
        assert!(matches!(joined.effects, EffectObservations::Unobserved(_)));
        if mutable_slot {
            assert!(
                matches!(&joined.result.slots().unwrap()[0].origins[0], DescriptorStoredOrigin::IncomingSlot { position: 0, path } if path.is_empty())
            );
        }
        drop(replay);
        let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
        assert!(analysis.flow(caller).is_ok());
        assert!(analysis.result_slots(callee).is_none());
        assert!(analysis.stored_effects(callee).is_none());
        drop(analysis);
        // A purported normal return without a registered result
        // carrier is unavailable analysis, not absent observations.
        graph[1].normal_exits = vec![at(0)];
        graph[1].returns = Some(vec![(at(0), location(99))]);
        let mut rejected =
            Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
        assert!(rejected.summary(callee, 0).is_none());
    }
}

#[test]
fn storage_access_frames_keep_helper_identity_and_unknown_calls() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowSource, DescriptorFlowStep,
    };
    use access_vocabulary::AccessOperation;
    use semantic_vocabulary::{BlockId, OperationId};

    let caller_id = MachineId::new(1).unwrap();
    let helper_id = MachineId::new(2).unwrap();
    let entry = (BlockId::new(1).unwrap(), 0);
    let parameter = location(1);
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let types = vec![
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::MutableBorrow),
    ];
    let body = |id| {
        let mut body = bare_machine(
            id,
            vec![DescriptorPlace {
                producer: None,
                place: parameter.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: reference,
                },
            }],
            BTreeMap::from([(parameter.root, reference)]),
            BTreeSet::new(),
        );
        body.flow.insert(entry, DescriptorFlowStep::default());
        body
    };
    let mut caller = body(caller_id);
    let mut helper = body(helper_id);
    let source = DescriptorFlowSource::Location(DescriptorLocation {
        root: parameter.root,
        path: vec![DescriptorPathSegment::Referent],
    });
    // An event need not observe the old descriptor value. Keep its
    // original operand index and distinguish read from write.
    helper.flow.get_mut(&entry).unwrap().inputs = vec![
        (Some(parameter.clone()), None),
        (None, Some((AccessOperation::Read, source.clone()))),
        (None, Some((AccessOperation::Write, source.clone()))),
    ];
    caller.calls.insert(
        entry,
        Some(vec![DescriptorInvocation {
            scalar_arguments: Vec::new(),
            owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
            target: DescriptorInvocationTarget::Machine(helper_id),
            arguments: vec![Some(parameter.clone())],
            argument_contents: vec![Some(DescriptorContentsQuery::from_location(
                &parameter,
            ))],
            result: None,
        }]),
    );
    let mut graph = vec![caller, helper];
    {
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
        // The caller points to the helper in the existing invocation
        // graph. Its summary does not copy the helper's event rows.
        assert_eq!(
            analysis.storage_access_frame(caller_id).unwrap(),
            Some([].as_slice())
        );
        assert_eq!(
            graph[0].known_invocation(&entry).unwrap().callee(),
            Some(helper_id)
        );
        let inputs = analysis
            .storage_access_frame(helper_id)
            .unwrap()
            .unwrap()
            .to_vec();
        assert_eq!(inputs, [(entry, 1), (entry, 2)]);
        for (input, operation) in inputs
            .into_iter()
            .zip([AccessOperation::Read, AccessOperation::Write])
        {
            assert!(matches!(analysis.storage_access(helper_id, input),
                Some((actual_operation, DescriptorFlowSource::Location(actual)))
                if actual_operation == operation
                    && matches!(&source, DescriptorFlowSource::Location(expected) if actual == expected)
            ));
            // Equal instruction and operand numbers in a different
            // machine cannot select the helper's memory operation.
            assert!(analysis.storage_access(caller_id, input).is_none());
        }
    }
    graph[0].calls.insert(entry, None);
    assert!(
        DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph)
            .storage_access_frame(caller_id)
            .unwrap()
            .is_none()
    );
    graph[0].calls.clear();
    assert_eq!(
        DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph)
            .storage_access_frame(caller_id)
            .unwrap(),
        Some([].as_slice())
    );
    // Knowing a possible implementation does not publish its private
    // accesses as the public requirement's checked effects.
    graph[0].calls.insert(
        entry,
        Some(vec![DescriptorInvocation {
            scalar_arguments: Vec::new(),
            owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
            target: DescriptorInvocationTarget::Public {
                effects_machine: Some(helper_id),
                realizations: vec![],
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                results: vec![],
                contents: Vec::new(),
                invalidates_contents: false,
            },
            arguments: vec![Some(parameter.clone())],
            argument_contents: vec![Some(DescriptorContentsQuery::from_location(
                &parameter,
            ))],
            result: None,
        }]),
    );
    assert!(
        DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph)
            .storage_access_frame(caller_id)
            .unwrap()
            .is_none()
    );
}

// An incomplete call leaves its callee summary unprepared. The
// caller's physical storage snapshot can nevertheless complete.
#[test]
fn late_return_summary_cannot_freeze_an_earlier_storage_query() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowEdge, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, OperationId};

    let caller_id = MachineId::new(1).unwrap();
    let callee_id = MachineId::new(2).unwrap();
    let block = BlockId::new(1).unwrap();
    let point = |index| (block, index);
    let edge = |target| DescriptorFlowEdge {
        effects: Vec::new(),
        edge: None,
        definitions: Vec::new(),
        target,
        parameters: Vec::new(),
        bindings: Vec::new(),
    };
    let scalar = scalar_type(1);
    let roster_id = StructuralTypeId::new(2).unwrap();
    let borrowed_id = StructuralTypeId::new(3).unwrap();
    let types = vec![
        scalar.clone(),
        StructuralTypeDeclaration {
            shape: StructuralTypeShape::ElementView {
                element: scalar.id,
                access: None,
            },
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            ..scalar_type(2)
        },
        StructuralTypeDeclaration {
            shape: StructuralTypeShape::ElementView {
                element: scalar.id,
                access: Some(StructuralAccess::SharedBorrow),
            },
            ..scalar_type(3)
        },
    ];
    let returned = location(1);
    let borrowed = location(2);
    let mut caller = bare_machine(
        caller_id,
        vec![
            DescriptorPlace {
                producer: Some(point(0)),
                place: returned.root,
                definition: DescriptorDefinition::Local,
            },
            DescriptorPlace {
                producer: Some(point(1)),
                place: borrowed.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::SharedBorrow,
                },
            },
        ],
        BTreeMap::from([(returned.root, roster_id), (borrowed.root, borrowed_id)]),
        BTreeSet::from([(point(1), returned.clone(), borrowed.clone())]),
    );
    caller.flow = BTreeMap::from([
        (
            point(0),
            DescriptorFlowStep {
                definitions: vec![returned.clone()],
                successors: vec![edge(point(1))],
                ..Default::default()
            },
        ),
        (
            point(1),
            DescriptorFlowStep {
                definitions: vec![borrowed.clone()],
                derivations: vec![(returned.clone(), borrowed.clone())],
                successors: vec![edge(point(2))],
                ..Default::default()
            },
        ),
        (point(2), DescriptorFlowStep::default()),
    ]);
    caller.normal_exits = vec![point(2)];
    caller
        .insert_call(
            point(0),
            DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
                target: DescriptorInvocationTarget::Machine(callee_id),
                arguments: vec![None],
                argument_contents: vec![None],
                result: Some(returned.clone()),
            },
        )
        .unwrap();

    let empty = location(1);
    let unused_input = location(2);
    let mut callee = bare_machine(
        callee_id,
        vec![
            DescriptorPlace {
                producer: Some(point(0)),
                place: empty.root,
                definition: DescriptorDefinition::Local,
            },
            DescriptorPlace {
                producer: None,
                place: unused_input.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::SharedBorrow,
                    structural_type: borrowed_id,
                },
            },
        ],
        BTreeMap::from([(empty.root, roster_id), (unused_input.root, borrowed_id)]),
        BTreeSet::new(),
    );
    callee.flow = BTreeMap::from([
        (
            point(0),
            DescriptorFlowStep {
                definitions: vec![empty.clone()],
                successors: vec![edge(point(1))],
                ..Default::default()
            },
        ),
        (point(1), DescriptorFlowStep::default()),
    ]);
    callee
        .originations
        .insert(point(0), vec![(empty.clone(), DescriptorCallOrigin::Empty)]);
    callee.normal_exits = vec![point(1)];
    callee.returns = Some(vec![(point(1), empty)]);
    let graph = vec![caller, callee];

    // Pin both original answers against one unchanged caller snapshot.
    let mut replay = Replay::new(&types, &graph);
    let snapshots = replay.states(caller_id, 0).unwrap();
    assert!(!replay.summaries.contains_key(&callee_id));
    let before = &snapshots[&point(2)];
    assert_eq!(
        replay.canonical_storage(&graph[0], before, &borrowed, None),
        Some(returned.clone()),
    );
    let summary = replay.summary(callee_id, 0).unwrap();
    assert!(matches!(
        summary.result.slots().map(Vec::as_slice),
        Some([slot]) if matches!(slot.origins.as_slice(),
            [DescriptorStoredOrigin::Empty])
    ));
    assert_eq!(
        replay.canonical_storage(&graph[0], before, &borrowed, None),
        Some(borrowed.clone()),
    );
    drop(replay);

    // Preparing another machine is not a graph edit; the memo must
    // not freeze an answer depending on that unavailable summary.
    let mut analysis = DescriptorAnalysis::new(&types, &graph);
    assert_eq!(
        analysis.location(caller_id, point(2), &borrowed),
        Some(returned.clone()),
    );
    assert_eq!(
        analysis.location(caller_id, point(2), &borrowed),
        Some(returned),
    );
    assert!(analysis.result_slots(callee_id).is_some());
    assert_eq!(
        analysis.location(caller_id, point(2), &borrowed),
        Some(borrowed.clone()),
    );
    assert_eq!(
        analysis.location(caller_id, point(2), &borrowed),
        Some(borrowed),
    );
}

#[test]
fn a_transferred_reference_keeps_its_write_crossing_until_caller_substitution() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowEdge, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, OperationId};
    let caller_id = MachineId::new(1).unwrap();
    let callee_id = MachineId::new(2).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let types = [
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::MutableBorrow),
    ];
    let entry = (BlockId::new(1).unwrap(), 0);
    let call_at = (entry.0, 1);
    let parameter = |place, structural_type, access| DescriptorPlace {
        producer: None,
        place,
        definition: DescriptorDefinition::Parameter {
            position: 0,
            structural_type,
            access,
        },
    };
    let mut caller = bare_machine(
        caller_id,
        vec![
            parameter(location(1).root, scalar, StructuralAccess::MutableBorrow),
            DescriptorPlace {
                producer: None,
                place: location(2).root,
                definition: DescriptorDefinition::Local,
            },
        ],
        BTreeMap::from([(location(1).root, scalar), (location(2).root, reference)]),
        BTreeSet::from([(entry, location(1), location(2))]),
    );
    caller.flow = BTreeMap::from([
        (
            entry,
            DescriptorFlowStep {
                definitions: vec![location(2)],
                derivations: vec![(location(1), location(2))],
                successors: vec![DescriptorFlowEdge {
                    definitions: Vec::new(),
                    edge: None,
                    effects: Vec::new(),
                    target: call_at,
                    parameters: Vec::new(),
                    bindings: Vec::new(),
                }],
                ..Default::default()
            },
        ),
        (call_at, DescriptorFlowStep::default()),
    ]);
    caller.normal_exits = vec![call_at];
    caller
        .insert_call(
            call_at,
            DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
                target: DescriptorInvocationTarget::Machine(callee_id),
                arguments: vec![Some(location(2))],
                argument_contents: vec![Some(DescriptorContentsQuery::from_location(
                    &location(2),
                ))],
                result: None,
            },
        )
        .unwrap();
    let mut callee = bare_machine(
        callee_id,
        vec![parameter(
            location(1).root,
            reference,
            StructuralAccess::Owned,
        )],
        BTreeMap::from([(location(1).root, reference)]),
        BTreeSet::new(),
    );
    let mut written = location(1);
    written.path.push(DescriptorPathSegment::Referent);
    callee.flow = BTreeMap::from([(
        entry,
        DescriptorFlowStep {
            definitions: vec![written.clone()],
            ..Default::default()
        },
    )]);
    let graph = [caller, callee];
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert_eq!(
        analysis.write_frame(caller_id).unwrap(),
        Some(vec![location(1)])
    );
}

#[test]
fn cleanup_actions_replay_in_order_at_their_real_edge_and_normal_exit() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowEdge, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, EdgeId};
    let identity = MachineId::new(1).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let types = [
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::MutableBorrow),
    ];
    let entry = (BlockId::new(1).unwrap(), 0);
    let left = (BlockId::new(2).unwrap(), 0);
    let right = (BlockId::new(3).unwrap(), 0);
    let edge = |raw| EdgeId::new(raw).unwrap();
    let owner = |raw, action_ordinal| DescriptorInvocationOwner::Cleanup {
        edge: edge(raw),
        action_ordinal,
    };
    let mut machine = bare_machine(
        identity,
        (1..=3)
            .enumerate()
            .map(|(position, raw)| DescriptorPlace {
                producer: None,
                place: location(raw).root,
                definition: DescriptorDefinition::Parameter {
                    position,
                    structural_type: reference,
                    access: StructuralAccess::MutableBorrow,
                },
            })
            .collect(),
        (1..=3).map(|raw| (location(raw).root, reference)).collect(),
        BTreeSet::new(),
    );
    machine.flow = BTreeMap::from([
        (
            entry,
            DescriptorFlowStep {
                successors: [(1, left), (2, right)]
                    .into_iter()
                    .map(|(raw, target)| DescriptorFlowEdge {
                        effects: Vec::new(),
                        edge: Some(edge(raw)),
                        target,
                        definitions: Vec::new(),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    })
                    .collect(),
                ..Default::default()
            },
        ),
        (left, DescriptorFlowStep::default()),
        (right, DescriptorFlowStep::default()),
    ]);
    machine.normal_exits = vec![left, right];
    let copy = |owner, destination, source| DescriptorInvocation {
        scalar_arguments: Vec::new(),
        owner,
        target: DescriptorInvocationTarget::Provider {
            results: Vec::new(),
            invalidates_contents: false,
            effects: vec![DescriptorStoredEffect {
                parameter: 0,
                steps: Vec::new(),
                origins: vec![DescriptorStoredOrigin::IncomingSlot {
                    position: 1,
                    path: Vec::new(),
                }],
            }],
        },
        arguments: vec![Some(location(destination)), Some(location(source))],
        argument_contents: vec![
            Some(DescriptorContentsQuery::from_location(&location(
                destination,
            ))),
            Some(DescriptorContentsQuery::from_location(&location(source))),
        ],
        result: None,
    };
    machine.insert_call(entry, copy(owner(1, 0), 1, 2)).unwrap();
    machine.insert_call(entry, copy(owner(1, 2), 3, 1)).unwrap();
    machine.insert_call(entry, copy(owner(2, 0), 3, 1)).unwrap();
    machine.insert_call(right, copy(owner(3, 0), 1, 2)).unwrap();
    let consumer_id = MachineId::new(2).unwrap();
    let mut consumer = bare_machine(
        consumer_id,
        vec![DescriptorPlace {
            producer: None,
            place: location(1).root,
            definition: DescriptorDefinition::Parameter {
                position: 0,
                structural_type: reference,
                access: StructuralAccess::SharedBorrow,
            },
        }],
        BTreeMap::from([(location(1).root, reference)]),
        BTreeSet::new(),
    );
    consumer.flow.insert(entry, DescriptorFlowStep::default());
    let observe = |owner| DescriptorInvocation {
        scalar_arguments: Vec::new(),
        owner,
        target: DescriptorInvocationTarget::Machine(consumer_id),
        arguments: vec![Some(location(1))],
        argument_contents: vec![Some(DescriptorContentsQuery::from_location(&location(1)))],
        result: None,
    };
    machine.insert_call(entry, observe(owner(1, 3))).unwrap();
    machine.insert_call(entry, observe(owner(2, 1))).unwrap();
    let graph = [machine, consumer];
    let mut replay = Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    replay.prepare_states(identity).unwrap();
    let expected = |raw| Some(BTreeSet::from([Origin::Incoming(location(raw))]));
    let value = |state: &State, raw| replay.read(&graph[0], state, &location(raw));
    assert_eq!(
        value(
            replay
                .invocation_state(identity, entry, owner(1, 0))
                .unwrap(),
            1
        ),
        expected(1)
    );
    assert_eq!(
        value(
            replay
                .invocation_state(identity, entry, owner(1, 2))
                .unwrap(),
            1
        ),
        expected(2)
    );
    assert_eq!(
        value(
            replay
                .invocation_state(identity, entry, owner(2, 0))
                .unwrap(),
            1
        ),
        expected(1)
    );
    let entries = &replay
        .snapshots
        .get(&identity)
        .unwrap()
        .ready()
        .unwrap()
        .before;
    assert_eq!(value(entries.get(&left).unwrap(), 3), expected(2));
    assert_eq!(value(entries.get(&right).unwrap(), 3), expected(1));
    assert_eq!(value(entries.get(&right).unwrap(), 1), expected(1));
    let completed = &replay
        .snapshots
        .get(&identity)
        .unwrap()
        .ready()
        .unwrap()
        .normal_exits;
    assert_eq!(value(completed.get(&right).unwrap(), 1), expected(2));
    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
    let incoming = DescriptorSourceOrigin::IncomingSlot(location(1));
    assert_eq!(
        analysis.substitute_call_source_at(identity, entry, owner(1, 3), &incoming),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(location(2))])
    );
    assert_eq!(
        analysis.substitute_call_source_at(identity, entry, owner(2, 1), &incoming),
        Some(vec![DescriptorSourceOrigin::IncomingSlot(location(1))])
    );
    assert!(
        analysis
            .substitute_call_source(identity, entry, &incoming)
            .is_none()
    );
    let mut referent = location(1);
    referent.path.push(DescriptorPathSegment::Referent);
    for (action, source) in [
        (owner(1, 3), location(2).root),
        (owner(2, 1), location(1).root),
    ] {
        assert_eq!(
            analysis
                .captured_storage_family_at(identity, entry, action, &referent)
                .unwrap(),
            vec![DescriptorCallOrigin::Argument {
                source,
                projection: Projection::new(&[StructuralPathSegment::Referent]),
            }],
            "cleanup must use its own prefix prestate at the shared position"
        );
    }
    assert_eq!(
        analysis.captured_storage_family_at(identity, entry, owner(1, 99), &referent),
        Err("captured storage action has no reachable prestate")
    );
    let flow = analysis.flow(identity).unwrap();
    let edges = &flow.get(&entry).unwrap().successors;
    assert_eq!(
        edges[0]
            .effects
            .iter()
            .map(|(owner, _)| *owner)
            .collect::<Vec<_>>(),
        [owner(1, 0), owner(1, 2), owner(1, 3)]
    );
    assert_eq!(
        edges[1]
            .effects
            .iter()
            .map(|(owner, _)| *owner)
            .collect::<Vec<_>>(),
        [owner(2, 0), owner(2, 1)]
    );
}

#[test]
fn a_backedge_cannot_make_a_replaced_descriptor_preserve_its_input() {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowEdge, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, OperationId};

    let caller_id = MachineId::new(1).unwrap();
    let callee_id = MachineId::new(2).unwrap();
    let view = StructuralTypeId::new(1).unwrap();
    let types = vec![StructuralTypeDeclaration {
        shape: StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            },
        ),
        ..scalar_type(1)
    }];
    let stored = location(1);
    let replacement = location(2);
    let entry = (BlockId::new(1).unwrap(), 0);
    let exit = (BlockId::new(2).unwrap(), 0);
    let replace = (BlockId::new(3).unwrap(), 0);
    let edge = |target| DescriptorFlowEdge {
        effects: Vec::new(),
        edge: None,
        definitions: Vec::new(),
        target,
        parameters: vec![],
        bindings: vec![],
    };
    let machine = |id| {
        bare_machine(
            id,
            [
                (&stored, StructuralAccess::MutableBorrow),
                (&replacement, StructuralAccess::SharedBorrow),
            ]
            .into_iter()
            .enumerate()
            .map(|(position, (place, access))| DescriptorPlace {
                producer: None,
                place: place.root,
                definition: DescriptorDefinition::Parameter {
                    position,
                    access,
                    structural_type: view,
                },
            })
            .collect(),
            BTreeMap::from([(stored.root, view), (replacement.root, view)]),
            BTreeSet::new(),
        )
    };
    let mut caller = machine(caller_id);
    caller.flow = BTreeMap::from([
        (
            entry,
            DescriptorFlowStep {
                successors: vec![edge(exit)],
                ..Default::default()
            },
        ),
        (exit, DescriptorFlowStep::default()),
    ]);
    caller.normal_exits = vec![exit];
    caller
        .insert_call(
            entry,
            DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
                target: DescriptorInvocationTarget::Machine(callee_id),
                arguments: vec![Some(stored.clone()), Some(replacement.clone())],
                argument_contents: vec![
                    Some(DescriptorContentsQuery::from_location(&stored)),
                    Some(DescriptorContentsQuery::from_location(&replacement)),
                ],
                result: None,
            },
        )
        .unwrap();
    let mut callee = machine(callee_id);
    callee.flow = BTreeMap::from([
        (
            entry,
            DescriptorFlowStep {
                successors: vec![edge(exit), edge(replace)],
                ..Default::default()
            },
        ),
        (exit, DescriptorFlowStep::default()),
        (
            replace,
            DescriptorFlowStep {
                definitions: vec![stored.clone()],
                derivations: vec![(replacement.clone(), stored.clone())],
                successors: vec![edge(entry)],
                ..Default::default()
            },
        ),
    ]);
    callee.normal_exits = vec![exit];
    let graph = vec![caller, callee];
    let mut replay =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::BorrowedStorage);
    let states = replay.states(callee_id, 0).unwrap();
    let initial = replay.initial(&graph[1]).unwrap();
    let original = replay.read(&graph[1], &initial, &stored).unwrap();
    let joined_entry = replay.read(&graph[1], &states[&entry], &stored).unwrap();
    let returned = replay.read(&graph[1], &states[&exit], &stored).unwrap();
    // Both routes reach the return. The fixed-point entry already
    // includes the replacement and cannot stand for invocation entry.
    assert_eq!(original.len(), 1);
    assert_eq!(joined_entry.len(), 2);
    assert_ne!(joined_entry, original);
    assert_eq!(joined_entry, returned);

    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert!(
        !analysis
            .call_preserves_descriptor_storage(caller_id, entry, 0, &[])
            .unwrap()
    );
}

#[test]
fn referent_contents_keep_the_pointer_backing_origin() {
    let scalar = StructuralTypeId::new(1).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let types = vec![
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::MutableBorrow),
    ];
    let parameter = location(1);
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![DescriptorPlace {
            producer: None,
            place: parameter.root,
            definition: DescriptorDefinition::Parameter {
                position: 0,
                access: StructuralAccess::MutableBorrow,
                structural_type: carrier,
            },
        }],
        BTreeMap::from([(parameter.root, carrier)]),
        BTreeSet::new(),
    );
    let replay = Replay::with_slot_kind(
        &types,
        std::slice::from_ref(&machine),
        DescriptorSlotKind::References,
    );
    let state = replay.initial(&machine).unwrap();
    let mut referent = parameter.clone();
    referent.path.push(DescriptorPathSegment::Referent);
    let query = DescriptorContentsQuery::from_location(&referent);
    let pointer = replay.read(&machine, &state, &parameter);
    assert_eq!(
        pointer,
        Some(BTreeSet::from([Origin::Incoming(parameter.clone())]))
    );
    assert_eq!(replay.read_contents(&machine, &state, &query), pointer);
}

#[test]
fn incoming_reference_aliases_keep_the_original_referent_and_refuse_ambiguity() {
    let scalar = StructuralTypeId::new(1).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let types = vec![
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::MutableBorrow),
    ];
    let original = location(1);
    let other = location(2);
    let returned = location(3);
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: original.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::Owned,
                    structural_type: carrier,
                },
            },
            DescriptorPlace {
                producer: None,
                place: other.root,
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::Owned,
                    structural_type: carrier,
                },
            },
            DescriptorPlace {
                producer: None,
                place: returned.root,
                definition: DescriptorDefinition::Local,
            },
        ],
        BTreeMap::from([
            (original.root, carrier),
            (other.root, carrier),
            (returned.root, carrier),
        ]),
        BTreeSet::new(),
    );
    let replay = Replay::with_slot_kind(
        &types,
        std::slice::from_ref(&machine),
        DescriptorSlotKind::References,
    );
    let mut state = replay.initial(&machine).unwrap();
    state.slots_mut().insert(
        returned.clone(),
        Some(BTreeSet::from([Origin::Incoming(original.clone())])),
    );
    let mut asked = returned.clone();
    asked.path.push(DescriptorPathSegment::Referent);
    let mut expected = original.clone();
    expected.path.push(DescriptorPathSegment::Referent);
    assert_eq!(
        replay.canonical_storage(&machine, &state, &asked, None),
        Some(expected.clone()),
    );
    let mut replacement = other.clone();
    replacement.path.push(DescriptorPathSegment::Referent);
    state.slots_mut().insert(
        original.clone(),
        Some(BTreeSet::from([Origin::Storage(replacement)])),
    );
    assert_eq!(
        replay.canonical_storage(&machine, &state, &asked, None),
        Some(expected),
    );
    let held = replay
        .resolve_storage(&machine, &state, &asked, None)
        .unwrap();
    let held = replay
        .checked_storage_family(&machine, &state, held)
        .unwrap();
    assert_eq!(
        held,
        vec![DescriptorCallOrigin::Argument {
            source: original.root,
            projection: Projection::new(&[StructuralPathSegment::Referent]),
        }],
        "captured entry lineage survives replacement of its descriptor home"
    );
    state.slots_mut().insert(
        returned.clone(),
        Some(BTreeSet::from([
            Origin::Incoming(original),
            Origin::Incoming(other),
        ])),
    );
    assert_eq!(
        replay.canonical_storage(&machine, &state, &asked, None),
        None
    );
    let alternatives = replay
        .resolve_storage(&machine, &state, &asked, None)
        .unwrap();
    assert_eq!(
        replay
            .checked_storage_family(&machine, &state, alternatives)
            .unwrap(),
        vec![
            DescriptorCallOrigin::Argument {
                source: location(1).root,
                projection: Projection::new(&[StructuralPathSegment::Referent])
            },
            DescriptorCallOrigin::Argument {
                source: location(2).root,
                projection: Projection::new(&[StructuralPathSegment::Referent])
            },
        ],
        "a complete incoming family preserves each original crossing"
    );
    state.slots_mut().insert(returned, None);
    let opaque = replay
        .resolve_storage(&machine, &state, &asked, None)
        .unwrap();
    assert!(
        replay
            .canonical_storage(&machine, &state, &asked, None)
            .is_some(),
        "the logical indirect coordinate is retained"
    );
    assert_eq!(
        replay.checked_storage_family(&machine, &state, opaque),
        Err("reference descriptor has no current origin account"),
        "a declaration does not prove completeness of the current unknown pointer"
    );
}

#[test]
fn exact_incoming_contents_keep_entry_identity_without_blessing_unknown_state() {
    let scalar = StructuralTypeId::new(1).unwrap();
    let inner = StructuralTypeId::new(2).unwrap();
    let outer = StructuralTypeId::new(3).unwrap();
    let types = vec![
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::SharedBorrow),
        reference_type(3, inner, StructuralAccess::SharedBorrow),
    ];
    let source = location(1);
    let held = location(2);
    let known = location(3);
    let unknown = location(4);
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: source.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::Owned,
                    structural_type: outer,
                },
            },
            DescriptorPlace {
                producer: None,
                place: held.root,
                definition: DescriptorDefinition::Local,
            },
            DescriptorPlace {
                producer: None,
                place: known.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::SharedBorrow,
                },
            },
            DescriptorPlace {
                producer: None,
                place: unknown.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::SharedBorrow,
                },
            },
        ],
        BTreeMap::from([
            (source.root, outer),
            (held.root, inner),
            (known.root, scalar),
            (unknown.root, scalar),
        ]),
        BTreeSet::new(),
    );
    let graph = [machine];
    let replay =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::BorrowedStorage);
    let machine = &graph[0];
    let mut state = replay.initial(machine).unwrap();
    let mut entry_descriptor = source.clone();
    entry_descriptor.path.push(DescriptorPathSegment::Referent);
    state.slots_mut().insert(
        held.clone(),
        Some(BTreeSet::from([Origin::IncomingContents(
            DescriptorContentsQuery::from_location(&entry_descriptor),
            Projection::new(&[]),
        )])),
    );
    let mut asked = held.clone();
    asked.path.push(DescriptorPathSegment::Referent);
    let captured = replay
        .resolve_storage(machine, &state, &asked, None)
        .unwrap();
    let expected = vec![DescriptorCallOrigin::Argument {
        source: source.root,
        projection: Projection::new(&[
            StructuralPathSegment::Referent,
            StructuralPathSegment::Referent,
        ]),
    }];
    assert_eq!(
        replay
            .checked_storage_family(machine, &state, captured.clone())
            .unwrap(),
        expected
    );
    // The held pointer is an actual retained incoming-content value.
    // Replacing its original descriptor cannot retarget that value.
    state.slots_mut().insert(source.clone(), None);
    let captured_after_replacement = replay
        .resolve_storage(machine, &state, &asked, None)
        .unwrap();
    assert_eq!(
        replay
            .checked_storage_family(machine, &state, captured_after_replacement)
            .unwrap(),
        expected
    );
    let mut current = entry_descriptor;
    current.path.push(DescriptorPathSegment::Referent);
    let opaque = replay
        .resolve_storage(machine, &state, &current, None)
        .unwrap();
    assert!(
        replay
            .canonical_storage(machine, &state, &current, None)
            .is_some(),
        "logical indirect coordinate remains representable"
    );
    assert!(
        replay
            .checked_storage_family(machine, &state, opaque.clone())
            .is_err(),
        "a current unknown pointer is not a captured entry value"
    );
    assert_eq!(
        replay.resolved(machine, replay.captured_storage_origins(captured.clone())),
        Some(expected.clone()),
        "a retained incoming value can publish its captured address"
    );
    let logical = Origin::Storage(opaque.local_coordinate().unwrap().clone());
    assert_eq!(
        replay.resolved(machine, replay.captured_storage_origins(opaque.clone())),
        replay.resolved(machine, BTreeSet::from([logical.clone()])),
        "unknown pointers retain their logical source for symbolic queries"
    );
    let parameter = |origin: &Origin| match replay.exported(machine, origin) {
        Some(DescriptorStoredOrigin::Parameter { position, projection }) => {
            Some((position, projection))
        }
        _ => None,
    };
    let expected_parameter = parameter(&logical).expect("the logical source names a parameter");
    assert_eq!(
        parameter(&Origin::CapturedStorage(opaque.clone())),
        Some(expected_parameter),
        "summary queries retain the same coordinate, without establishing a capture"
    );
    Arc::make_mut(&mut state.arrival_backing).insert(
        known.clone(),
        Some(replay.captured_storage_origins(captured)),
    );
    Arc::make_mut(&mut state.arrival_backing).insert(
        unknown.clone(),
        Some(replay.captured_storage_origins(opaque)),
    );
    state.slots_mut().insert(
        source.clone(),
        Some(BTreeSet::from([Origin::Incoming(source)])),
    );
    let known_family = replay
        .resolve_storage(machine, &state, &known, None)
        .unwrap();
    assert_eq!(
        replay
            .checked_storage_family(machine, &state, known_family)
            .unwrap(),
        expected
    );
    let unknown_family = replay
        .resolve_storage(machine, &state, &unknown, None)
        .unwrap();
    assert!(
        replay
            .checked_storage_family(machine, &state, unknown_family)
            .is_err(),
        "forwarding and a later replacement cannot upgrade an unknown capture"
    );
    let mut mixed = state.arrival_backing.get(&known).unwrap().clone().unwrap();
    mixed.extend(
        state
            .arrival_backing
            .get(&unknown)
            .unwrap()
            .clone()
            .unwrap(),
    );
    Arc::make_mut(&mut state.arrival_backing).insert(known.clone(), Some(mixed));
    let mixed = replay
        .resolve_storage(machine, &state, &known, None)
        .unwrap();
    assert!(
        replay
            .checked_storage_family(machine, &state, mixed)
            .is_err(),
        "one unknown arrival poisons the family even when its logical coordinate coincides"
    );
}

#[test]
fn projected_reference_families_keep_the_declared_referee_type() {
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let record = StructuralTypeId::new(3).unwrap();
    let types = vec![
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::SharedBorrow),
        StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
            ..scalar_type(3)
        },
    ];
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: location(1).root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::SharedBorrow,
                    structural_type: scalar,
                },
            },
            DescriptorPlace {
                producer: None,
                place: location(2).root,
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::SharedBorrow,
                    structural_type: record,
                },
            },
            DescriptorPlace {
                producer: None,
                place: location(3).root,
                definition: DescriptorDefinition::Local,
            },
        ],
        BTreeMap::from([
            (location(1).root, scalar),
            (location(2).root, record),
            (location(3).root, reference),
        ]),
        BTreeSet::new(),
    );
    let graph = [machine];
    let replay =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::BorrowedStorage);
    let machine = &graph[0];
    let mut asked = location(3);
    asked.path.push(DescriptorPathSegment::Referent);
    for retained in [false, true] {
        let mut state = replay.initial(machine).unwrap();
        let origin = |source| {
            let address = DescriptorCallOrigin::Argument {
                source,
                projection: Projection::new(&[]),
            };
            if retained {
                Origin::CapturedStorage(StorageResolution::Projected(address))
            } else {
                Origin::Backing(address)
            }
        };
        state.slots_mut().insert(
            location(3),
            Some(BTreeSet::from([origin(location(1).root)])),
        );
        let valid = replay
            .resolve_storage(machine, &state, &asked, None)
            .unwrap();
        assert_eq!(
            replay
                .checked_storage_family(machine, &state, valid)
                .unwrap(),
            vec![DescriptorCallOrigin::Argument {
                source: location(1).root,
                projection: Projection::new(&[])
            }]
        );
        state.slots_mut().insert(
            location(3),
            Some(BTreeSet::from([origin(location(2).root)])),
        );
        let invalid = replay
            .resolve_storage(machine, &state, &asked, None)
            .unwrap();
        let refusal = replay
            .checked_storage_family(machine, &state, invalid)
            .unwrap_err();
        assert!(refusal.contains("declared referee type"), "{refusal}");
    }
}

#[test]
fn an_indexed_reference_arrival_keeps_address_and_contents_separate() {
    use semantic_vocabulary::{BlockId, ObligationId, ValueId};
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let view = StructuralTypeId::new(3).unwrap();
    let types = [
        scalar_type(1),
        reference_type(2, scalar, StructuralAccess::SharedBorrow),
        StructuralTypeDeclaration {
            shape: StructuralTypeShape::ElementView {
                element: reference,
                access: Some(StructuralAccess::MutableBorrow),
            },
            ..scalar_type(3)
        },
    ];
    let captured = location(4);
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![DescriptorPlace {
            producer: None,
            place: captured.root,
            definition: DescriptorDefinition::Arrival {
                access: StructuralAccess::MutableBorrow,
            },
        }],
        BTreeMap::from([
            (location(1).root, view),
            (location(2).root, scalar),
            (location(3).root, scalar),
            (captured.root, reference),
        ]),
        BTreeSet::new(),
    );
    let graph = [machine];
    let replay =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::BorrowedStorage);
    let address = DescriptorCallOrigin::Argument {
        source: location(1).root,
        projection: Projection::new(&[StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(7).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        }])
        .at((BlockId::new(1).unwrap(), 0)),
    };
    let mut state = State::default();
    Arc::make_mut(&mut state.arrival_backing).insert(
        captured.clone(),
        Some(BTreeSet::from([Origin::Backing(address.clone())])),
    );
    for referent in [location(2), location(3)] {
        let contents = BTreeSet::from([Origin::Storage(referent)]);
        state
            .slots_mut()
            .insert(captured.clone(), Some(contents.clone()));
        assert_eq!(replay.read(&graph[0], &state, &captured), Some(contents));
        assert!(matches!(
            replay.resolve_storage(&graph[0], &state, &captured, None),
            Some(StorageResolution::Projected(held)) if held == address
        ));
    }
}

#[test]
fn indexed_arrival_captures_backing_before_descriptor_replacement() {
    for contents_available in [true, false] {
        check_indexed_arrival_capture(contents_available);
    }
}

fn check_indexed_arrival_capture(contents_available: bool) {
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, ObligationId, ValueId};
    let scalar = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let types = vec![
        scalar_type(1),
        StructuralTypeDeclaration {
            shape: StructuralTypeShape::ElementView {
                element: scalar,
                access: Some(StructuralAccess::MutableBorrow),
            },
            ..scalar_type(2)
        },
    ];
    let source = location(1);
    let replacement = location(2);
    let captured = location(3);
    let descriptor_alias = location(4);
    let forwarded = location(5);
    let entry = (BlockId::new(1).unwrap(), 0);
    let target = (BlockId::new(2).unwrap(), 0);
    let forwarded_target = (BlockId::new(3).unwrap(), 0);
    let mut machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: source.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: view,
                },
            },
            DescriptorPlace {
                producer: None,
                place: replacement.root,
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: view,
                },
            },
            DescriptorPlace {
                producer: None,
                place: captured.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::MutableBorrow,
                },
            },
            DescriptorPlace {
                producer: None,
                place: descriptor_alias.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::MutableBorrow,
                },
            },
            DescriptorPlace {
                producer: None,
                place: forwarded.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::MutableBorrow,
                },
            },
        ],
        BTreeMap::from([
            (source.root, view),
            (replacement.root, view),
            (captured.root, scalar),
            (descriptor_alias.root, view),
            (forwarded.root, scalar),
        ]),
        BTreeSet::new(),
    );
    let index = StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(7).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    };
    let projection = Projection::new(std::slice::from_ref(&index)).at(entry);
    let mut binding = DescriptorFlowBinding::new(
        DescriptorFlowSource::from_query(
            source.root,
            Some(DescriptorContentsQuery {
                root: source.root,
                steps: vec![
                    crate::descriptor_lifetimes::DescriptorContentsStep::any_element(
                        scalar,
                    ),
                ],
            }),
        ),
        captured.clone(),
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    )
    .unwrap();
    binding.captured_storage = Some((source.clone(), projection.clone()));
    machine.flow = BTreeMap::from([
        (
            entry,
            DescriptorFlowStep {
                successors: vec![DescriptorFlowEdge {
                    edge: None,
                    effects: Vec::new(),
                    definitions: Vec::new(),
                    target,
                    parameters: vec![captured.clone(), descriptor_alias.clone()],
                    bindings: vec![
                        binding,
                        DescriptorFlowBinding::new(
                            source.clone(),
                            descriptor_alias.clone(),
                            StructuralAccess::MutableBorrow,
                            StructuralAccess::MutableBorrow,
                        )
                        .unwrap(),
                    ],
                }],
                ..Default::default()
            },
        ),
        (
            target,
            DescriptorFlowStep {
                successors: vec![DescriptorFlowEdge {
                    edge: None,
                    effects: Vec::new(),
                    definitions: Vec::new(),
                    target: forwarded_target,
                    parameters: vec![forwarded.clone()],
                    bindings: vec![
                        DescriptorFlowBinding::new(
                            captured.clone(),
                            forwarded.clone(),
                            StructuralAccess::MutableBorrow,
                            StructuralAccess::MutableBorrow,
                        )
                        .unwrap(),
                    ],
                }],
                ..DescriptorFlowStep::default()
            },
        ),
        (forwarded_target, DescriptorFlowStep::default()),
    ]);
    let mut graph = vec![machine];
    if !contents_available {
        graph[0].flow.get_mut(&entry).unwrap().successors[0].bindings[0].source =
            DescriptorFlowSource::Unavailable(source.root);
    }
    let mut replay =
        Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::BorrowedStorage);
    let states = replay.states(graph[0].id, 0).unwrap();
    let expected = DescriptorCallOrigin::Argument {
        source: source.root,
        projection: projection.clone(),
    };
    assert!(
        matches!(
            replay.resolve_storage(&graph[0], &states[&forwarded_target], &forwarded, None),
            Some(StorageResolution::Projected(held)) if held == expected
        ),
        "a second borrowed-header edge preserves the original captured projection"
    );
    let mut after = states[&target].clone();
    assert!(
        matches!(
            replay.resolve_storage(&graph[0], &after, &descriptor_alias, None),
            Some(StorageResolution::Unique(_))
        ),
        "descriptor value transport must remain distinct from captured referent storage"
    );
    // Replacing the descriptor home cannot retarget its captured child.
    let new_origin = after.slots.get(&replacement).unwrap().clone();
    after.slots_mut().insert(source.clone(), new_origin);
    after.slots_mut().insert(
        captured.clone(),
        Some(BTreeSet::from([Origin::Storage(replacement.clone())])),
    );
    let StorageResolution::Projected(DescriptorCallOrigin::Argument {
        source: original,
        projection: held,
    }) = replay
        .resolve_storage(&graph[0], &after, &captured, None)
        .unwrap()
    else {
        panic!("indexed arrival must retain its original projected backing");
    };
    assert_eq!(original, source.root);
    assert_eq!(held, projection);
    assert_eq!(held.captured_storage_path(), Some(vec![index.clone()]));
    assert!(
        Projection::new(std::slice::from_ref(&index))
            .captured_storage_path()
            .is_none()
    );
    assert!(
        held.clone()
            .without_runtime_coordinates()
            .captured_storage_path()
            .is_none()
    );
    let mut missing = after.clone();
    Arc::make_mut(&mut missing.arrival_backing).insert(captured.clone(), None);
    let mut joined_missing = after.clone();
    assert!(merge_state(&mut joined_missing, &missing));
    assert!(
        replay
            .resolve_storage(&graph[0], &joined_missing, &captured, None)
            .is_none()
    );
    Arc::make_mut(&mut after.arrival_backing).insert(
        captured.clone(),
        Some(BTreeSet::from([
            Origin::Backing(DescriptorCallOrigin::Argument {
                source: source.root,
                projection: held.clone(),
            }),
            Origin::Backing(DescriptorCallOrigin::Argument {
                source: replacement.root,
                projection: held,
            }),
        ])),
    );
    assert!(matches!(
        replay.resolve_storage(&graph[0], &after, &captured, None),
        Some(StorageResolution::Alternatives(_))
    ));
    let case_storage = super::case_flow::Storage::new(&replay, &graph[0]);
    let edge = &graph[0].flow[&target].successors[0];
    for (alternative, before) in [&joined_missing, &after].into_iter().enumerate() {
        let arrived = replay
            .transfer_edge(
                &graph[0],
                before,
                before,
                edge,
                &case_storage,
                &DescriptorReadDepth::at(&graph[0], before, None),
            )
            .unwrap();
        let resolution = replay.resolve_storage(&graph[0], &arrived, &forwarded, None);
        if alternative == 0 {
            assert!(
                resolution.is_none(),
                "forwarding cannot establish missing backing"
            );
        } else {
            // Complete alternatives now survive forwarding. They are
            // not one backing from which a singular query may choose.
            let family = replay
                .checked_storage_family(&graph[0], &arrived, resolution.unwrap())
                .unwrap();
            assert_eq!(
                family,
                vec![
                    DescriptorCallOrigin::Argument {
                        source: source.root,
                        projection: projection.clone()
                    },
                    DescriptorCallOrigin::Argument {
                        source: replacement.root,
                        projection: projection.clone()
                    },
                ]
            );
        }
    }
    drop(replay);
    let mut other = graph[0].flow[&entry].successors[0].clone();
    other.bindings[0].captured_storage.as_mut().unwrap().0 = replacement.clone();
    graph[0]
        .flow
        .get_mut(&entry)
        .unwrap()
        .successors
        .push(other);
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    for (point, location) in [(target, &captured), (forwarded_target, &forwarded)] {
        assert_eq!(
            analysis
                .captured_storage_family(graph[0].id, point, location)
                .unwrap()
                .len(),
            2
        );
        assert!(
            analysis
                .storage_projection(graph[0].id, point, location)
                .is_none(),
            "forwarding cannot establish one backing from competing captures"
        );
    }
}

/// Replay the two actual edge bindings that a mixed input/captured
/// arrival produces, then forward the same address through another header.
fn mixed_storage_arrival() -> (Vec<StructuralTypeDeclaration>, DescriptorMachine) {
    use crate::descriptor_lifetimes::{
        DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, DescriptorFlowStep,
    };
    use semantic_vocabulary::{BlockId, ObligationId, ValueId};
    let scalar = StructuralTypeId::new(1).unwrap();
    let view = StructuralTypeId::new(2).unwrap();
    let types = vec![
        scalar_type(1),
        StructuralTypeDeclaration {
            shape: StructuralTypeShape::ElementView {
                element: scalar,
                access: Some(StructuralAccess::SharedBorrow),
            },
            ..scalar_type(2)
        },
    ];
    let source = location(1);
    let independent = location(2);
    let captured = location(3);
    let forwarded = location(4);
    let entry = (BlockId::new(1).unwrap(), 0);
    let join = (BlockId::new(2).unwrap(), 0);
    let exit = (BlockId::new(3).unwrap(), 0);
    let mut machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: source.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::SharedBorrow,
                    structural_type: view,
                },
            },
            DescriptorPlace {
                producer: None,
                place: independent.root,
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::SharedBorrow,
                    structural_type: scalar,
                },
            },
            DescriptorPlace {
                producer: None,
                place: captured.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::SharedBorrow,
                },
            },
            DescriptorPlace {
                producer: None,
                place: forwarded.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::SharedBorrow,
                },
            },
        ],
        BTreeMap::from([
            (source.root, view),
            (independent.root, scalar),
            (captured.root, scalar),
            (forwarded.root, scalar),
        ]),
        BTreeSet::new(),
    );
    let mut indexed = DescriptorFlowBinding::new(
        DescriptorFlowSource::from_query(
            source.root,
            Some(DescriptorContentsQuery {
                root: source.root,
                steps: vec![
                    crate::descriptor_lifetimes::DescriptorContentsStep::any_element(
                        scalar,
                    ),
                ],
            }),
        ),
        captured.clone(),
        StructuralAccess::SharedBorrow,
        StructuralAccess::SharedBorrow,
    )
    .unwrap();
    indexed.captured_storage = Some((
        source.clone(),
        Projection::new(&[StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(7).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        }])
        .at(entry),
    ));
    let edge = |target, parameter: DescriptorLocation, binding| DescriptorFlowEdge {
        edge: None,
        effects: Vec::new(),
        definitions: Vec::new(),
        target,
        parameters: vec![parameter],
        bindings: vec![binding],
    };
    machine.flow = BTreeMap::from([
        (
            entry,
            DescriptorFlowStep {
                successors: vec![
                    edge(join, captured.clone(), indexed),
                    edge(
                        join,
                        captured.clone(),
                        DescriptorFlowBinding::new(
                            independent,
                            captured.clone(),
                            StructuralAccess::SharedBorrow,
                            StructuralAccess::SharedBorrow,
                        )
                        .unwrap(),
                    ),
                ],
                ..Default::default()
            },
        ),
        (
            join,
            DescriptorFlowStep {
                inputs: vec![(Some(captured.clone()), None)],
                successors: vec![edge(
                    exit,
                    forwarded.clone(),
                    DescriptorFlowBinding::new(
                        captured,
                        forwarded,
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow,
                    )
                    .unwrap(),
                )],
                ..Default::default()
            },
        ),
        (exit, DescriptorFlowStep::default()),
    ]);
    machine.normal_exits = vec![exit];
    (types, machine)
}

#[test]
fn complete_storage_family_keeps_mixed_arrivals_and_forwarding() {
    let (types, machine) = mixed_storage_arrival();
    let graph = [machine];
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    let entry = graph[0].entry;
    let expected = BTreeSet::from([
        DescriptorCallOrigin::Argument {
            source: location(1).root,
            projection: graph[0].flow[&entry].successors[0].bindings[0]
                .captured_storage
                .as_ref()
                .unwrap()
                .1
                .clone(),
        },
        DescriptorCallOrigin::Argument {
            source: location(2).root,
            projection: Projection::new(&[]),
        },
    ])
    .into_iter()
    .collect::<Vec<_>>();
    let flow = analysis
        .flow(graph[0].id)
        .expect("complete addresses retain the local reference variable");
    let join = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
    assert!(
        flow[&join]
            .observed_inputs()
            .any(|input| *input == location(3))
    );
    for (block, descriptor) in [(2, location(3)), (3, location(4))] {
        let point = (semantic_vocabulary::BlockId::new(block).unwrap(), 0);
        let actual = analysis
            .captured_storage_family(graph[0].id, point, &descriptor)
            .unwrap();
        assert_eq!(
            actual, expected,
            "every real edge's captured address must survive"
        );
        assert_eq!(
            analysis
                .captured_storage_family(graph[0].id, point, &descriptor)
                .unwrap(),
            actual,
            "the existing query memo must preserve the complete family"
        );
        assert!(analysis.location(graph[0].id, point, &descriptor).is_none());
        assert!(
            analysis
                .storage_projection(graph[0].id, point, &descriptor)
                .is_none(),
            "a complete family is not one exact incoming loan"
        );
    }
}

#[test]
fn a_returned_element_family_keeps_its_slice_bounds_and_requires_a_live_capture() {
    let scalar = scalar_type(1);
    let array = StructuralTypeDeclaration {
        shape: StructuralTypeShape::FixedArray {
            element: scalar.id,
            length: terminal_psi::ArrayExtent::Literal(8),
        },
        ..scalar_type(2)
    };
    let types = [scalar, array];
    let root = location(1).root;
    let at = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
    let variable = semantic_vocabulary::ValueId::new(3).unwrap();
    let window = Projection::window(
        semantic_vocabulary::ValueId::new(1).unwrap(),
        semantic_vocabulary::ValueId::new(2).unwrap(),
        at,
    );
    let indexed = Projection::new(&[StructuralPathSegment::RuntimeIndex {
        index: variable,
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
    }]);
    let literals = |value: semantic_vocabulary::ValueId| match value.get() {
        1 => Some(2),
        2 => Some(5),
        _ => None,
    };
    let array_id = types[1].id;
    let result = window
        .then(&indexed.clone().at(at))
        .without_runtime_coordinates_with(literals, |position| position == at);
    assert!(result.captured_address_path().is_err());
    assert_eq!(
        result.returned_element_addresses(&types, root, array_id),
        Some(
            (2..5)
                .map(|index| vec![StructuralPathSegment::FixedIndex(index)])
                .collect()
        )
    );
    for missing in [
        window.then(&indexed),
        window.then(&indexed.clone().at((at.0, 1))),
        indexed.at(at).without_captured_address(),
    ] {
        assert!(
            missing
                .without_runtime_coordinates_with(literals, |position| position == at)
                .returned_element_addresses(&types, root, array_id)
                .is_none()
        );
    }
}

/// `slice[index] = value` stores through a header bound to one element. The
/// header's backing is a captured address into the collection, so the store
/// may have reached any member of a fixed collection and nothing outside it.
#[test]
fn a_store_through_a_captured_element_names_every_member_of_a_fixed_collection() {
    use semantic_vocabulary::{BlockId, ObligationId, ValueId};
    let element = scalar_type(1);
    let fixed = StructuralTypeDeclaration {
        shape: StructuralTypeShape::FixedArray {
            element: element.id,
            length: terminal_psi::ArrayExtent::Literal(2),
        },
        ..scalar_type(2)
    };
    // A slice has no member list to enumerate.
    let open = StructuralTypeDeclaration {
        shape: StructuralTypeShape::ElementView {
            element: element.id,
            access: Some(StructuralAccess::MutableBorrow),
        },
        ..scalar_type(2)
    };
    let owner = location(1);
    let header = location(4);
    let at = (BlockId::new(1).unwrap(), 0);
    let indexed = Projection::new(&[StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(7).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    }]);
    let window = Projection::window(ValueId::new(8).unwrap(), ValueId::new(9).unwrap(), at);
    let members = (0..2)
        .map(|index| DescriptorLocation {
            root: owner.root,
            path: vec![DescriptorPathSegment::FixedIndex(index)],
        })
        .collect::<Vec<_>>();
    for (collection, projection, expected) in [
        (fixed.clone(), indexed.clone().at(at), Some(members)),
        (open, indexed.clone().at(at), None),
        (fixed.clone(), window.then(&indexed.clone().at(at)), None),
        (fixed, indexed.at((at.0, 1)), None),
    ] {
        let types = [element.clone(), collection];
        let mut machine = bare_machine(
            MachineId::new(1).unwrap(),
            vec![
                DescriptorPlace {
                    producer: None,
                    place: owner.root,
                    definition: DescriptorDefinition::Local,
                },
                DescriptorPlace {
                    producer: None,
                    place: header.root,
                    definition: DescriptorDefinition::Arrival {
                        access: StructuralAccess::MutableBorrow,
                    },
                },
            ],
            BTreeMap::from([(owner.root, types[1].id), (header.root, types[0].id)]),
            BTreeSet::new(),
        );
        machine.flow.insert(at, Default::default());
        let graph = [machine];
        let replay = Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::BorrowedStorage);
        let mut state = State::default();
        state.slots_mut().insert(
            owner.clone(),
            Some(BTreeSet::from([Origin::Storage(owner.clone())])),
        );
        Arc::make_mut(&mut state.arrival_backing).insert(
            header.clone(),
            Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::Argument {
                    source: owner.root,
                    projection,
                },
            )])),
        );
        assert_eq!(replay.storage_origins(&graph[0], &state, &header), expected);
    }
}

#[test]
fn complete_storage_family_refuses_missing_capture_unknown_and_stale_inputs() {
    let (types, machine) = mixed_storage_arrival();
    let point = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
    let captured = location(3);
    for mutation in 0..5 {
        let (_, mut changed) = mixed_storage_arrival();
        match mutation {
            0 => {
                let (_, projection) =
                    changed.flow.get_mut(&changed.entry).unwrap().successors[0].bindings[0]
                        .captured_storage
                        .as_mut()
                        .unwrap();
                *projection = Projection::new(&projection.path);
            }
            1 => {
                let (_, projection) =
                    changed.flow.get_mut(&changed.entry).unwrap().successors[0].bindings[0]
                        .captured_storage
                        .as_mut()
                        .unwrap();
                *projection = projection.clone().without_runtime_coordinates();
            }
            2 => {
                changed.flow.get_mut(&changed.entry).unwrap().successors[1].bindings[0]
                    .source =
                    crate::descriptor_lifetimes::DescriptorFlowSource::Unavailable(
                        location(2).root,
                    );
            }
            3 => {
                let (_, projection) =
                    changed.flow.get_mut(&changed.entry).unwrap().successors[0].bindings[0]
                        .captured_storage
                        .as_mut()
                        .unwrap();
                *projection = Projection::new(&projection.path)
                    .at((semantic_vocabulary::BlockId::new(99).unwrap(), 0));
            }
            _ => {
                changed.flow.get_mut(&changed.entry).unwrap().successors[0].bindings[0]
                    .captured_storage
                    .as_mut()
                    .unwrap()
                    .0 = location(99);
            }
        }
        let graph = [changed];
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
        assert!(
            analysis
                .captured_storage_family(graph[0].id, point, &captured)
                .is_err(),
            "mutation {mutation} cannot return only the known sibling or an invented capture"
        );
        assert!(
            analysis
                .storage_projection(graph[0].id, point, &captured)
                .is_none(),
            "mutation {mutation} cannot establish a singular captured address"
        );
        assert!(
            analysis.flow(graph[0].id).is_err(),
            "mutation {mutation} cannot establish reference flow"
        );
    }
    let graph = [machine];
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert_eq!(
        analysis.captured_storage_family(MachineId::new(99).unwrap(), point, &captured),
        Err("captured storage machine is absent or stale")
    );
    assert_eq!(
        analysis.captured_storage_family(
            graph[0].id,
            (semantic_vocabulary::BlockId::new(99).unwrap(), 0),
            &captured
        ),
        Err("captured storage position has no reachable snapshot")
    );
}

#[test]
fn storage_queries_reject_duplicate_machine_ids() {
    let (types, first) = mixed_storage_arrival();
    let (_, second) = mixed_storage_arrival();
    let id = first.id;
    let graph = [first, second];
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    let point = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
    let captured = location(3);
    assert_eq!(
        analysis.captured_storage_family(id, point, &captured),
        Err("captured storage machine is absent or stale")
    );
    assert_eq!(analysis.location(id, point, &captured), None);
    assert_eq!(
        analysis.flow(id).err(),
        Some(DescriptorGraphError::MissingOrAmbiguousMachine(id))
    );
}

#[test]
fn branch_selected_storage_keeps_every_backing_origin() {
    let tag = StructuralTypeId::new(1).unwrap();
    let empty = semantic_vocabulary::StructuralCaseId::new(1).unwrap();
    let present = semantic_vocabulary::StructuralCaseId::new(2).unwrap();
    let types = [StructuralTypeDeclaration {
        shape: StructuralTypeShape::Sum {
            cases: [empty, present].map(|id| terminal_psi::StructuralCaseDeclaration {
                declaration_identity: Default::default(),
                id,
                identity: terminal_psi::TerminalIdentityIndex(id.get() as u32),
                fields: Vec::new(),
            }).to_vec(),
        },
        ..scalar_type(1)
    }];
    let left = location(1);
    let right = location(2);
    let selected = location(3);
    let captured = location(4);
    let position = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
    let machine = bare_machine(
        MachineId::new(1).unwrap(),
        vec![
            DescriptorPlace {
                producer: None,
                place: left.root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: tag,
                },
            },
            DescriptorPlace {
                producer: None,
                place: right.root,
                definition: DescriptorDefinition::Parameter {
                    position: 1,
                    access: StructuralAccess::MutableBorrow,
                    structural_type: tag,
                },
            },
            DescriptorPlace {
                producer: None,
                place: selected.root,
                definition: DescriptorDefinition::Arrival {
                    access: StructuralAccess::MutableBorrow,
                },
            },
            DescriptorPlace {
                producer: None,
                place: captured.root,
                definition: DescriptorDefinition::Local,
            },
        ],
        BTreeMap::from([
            (left.root, tag),
            (right.root, tag),
            (selected.root, tag),
            (captured.root, tag),
        ]),
        BTreeSet::from([(position, selected.clone(), captured.clone())]),
    );
    let replay = Replay::with_slot_kind(
        &types,
        std::slice::from_ref(&machine),
        DescriptorSlotKind::References,
    );
    let origins = BTreeSet::from([
        Origin::Storage(left.clone()),
        Origin::Storage(right.clone()),
    ]);
    let mut before = State::default();
    Arc::make_mut(&mut before.arrival_backing).insert(selected.clone(), Some(origins));
    let case_storage = super::case_flow::Storage::new(&replay, &machine);
    let facts = [&left, &right].map(|place| DescriptorCasePredicate {
        subject: DescriptorFlowSource::Location(place.clone()),
        case: empty,
    });
    for fact in &facts {
        super::case_flow::establish(&replay, &machine, &mut before, fact).unwrap();
    }
    let after = replay
        .transfer(
            &machine,
            &before,
            std::slice::from_ref(&captured),
            &[],
            [(&selected, &captured)].into_iter(),
            super::case_flow::Transfer::Operation {
                position,
                reads: &DescriptorReadDepth::at(&machine, &before, None),
            },
            &case_storage,
        )
        .unwrap();
    assert!(matches!(
        replay.resolve_storage(&machine, &before, &selected, None),
        Some(StorageResolution::Alternatives(_))
    ));
    // A family keeps complete address accounts for both inputs.
    // Capturing it must neither choose one input nor invent a new home.
    let held = BTreeSet::from([
        Origin::Backing(DescriptorCallOrigin::Argument {
            source: left.root,
            projection: Projection::new(&[]),
        }),
        Origin::Backing(DescriptorCallOrigin::Argument {
            source: right.root,
            projection: Projection::new(&[]),
        }),
    ]);
    assert_eq!(after.slots.get(&captured), Some(&Some(held.clone())));
    assert_eq!(replay.read(&machine, &after, &captured), Some(held));

    // A store through the choice can change either candidate's tag.
    // Neither earlier membership can justify a later dispatch.
    for fact in &facts {
        assert!(super::case_flow::established(&replay, &machine, &before, fact));
    }
    let written = replay.transfer(
        &machine,
        &before,
        std::slice::from_ref(&selected),
        &[],
        std::iter::empty(),
        super::case_flow::Transfer::Operation {
            position,
            reads: &DescriptorReadDepth::at(&machine, &before, None),
        },
        &case_storage,
    ).unwrap();
    for fact in &facts {
        assert!(!super::case_flow::established(&replay, &machine, &written, fact));
    }
}
