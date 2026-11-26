use super::*;
use semantic_vocabulary::{BlockId, PlaceId};

fn position(ordinal: u32) -> DescriptorPosition {
    (BlockId::new(1).unwrap(), ordinal)
}

fn root(number: u64) -> DescriptorLocation {
    DescriptorLocation::whole(PlaceId::new(number).unwrap())
}

fn edge(target: u32) -> DescriptorFlowEdge {
    DescriptorFlowEdge {
        edge: None,
        effects: Vec::new(),
        definitions: Vec::new(),
        target: position(target),
        parameters: Vec::new(),
        bindings: Vec::new(),
    }
}

#[test]
fn every_edge_selects_its_actual_parent_even_with_the_same_target() {
    let selected = root(3);
    let left = ObligationId::new(1).unwrap();
    let right = ObligationId::new(2).unwrap();
    let left_edge = EdgeId::new(1).unwrap();
    let right_edge = EdgeId::new(2).unwrap();
    let from = position(0);
    let to = position(1);
    let mut steps = BTreeMap::from([
        (
            from,
            DescriptorFlowStep {
                successors: [(left_edge, root(1)), (right_edge, root(2))]
                    .into_iter()
                    .map(|(identity, source)| {
                        let mut selected_edge = edge(1);
                        selected_edge.edge = Some(identity);
                        selected_edge.parameters.push(selected.clone());
                        selected_edge.bindings.push(
                            DescriptorFlowBinding::new(
                                DescriptorFlowSource::Location(source),
                                selected.clone(),
                                terminal_psi::StructuralAccess::Owned,
                                terminal_psi::StructuralAccess::Owned,
                            )
                            .unwrap(),
                        );
                        selected_edge
                    })
                    .collect(),
                ..Default::default()
            },
        ),
        (
            to,
            DescriptorFlowStep {
                inputs: vec![(Some(selected.clone()), None)],
                ..Default::default()
            },
        ),
    ]);
    let start = (to, selected.clone());
    let choice = |steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>| {
        let left_life = descriptor_entry_lifetime(steps, from, &root(1));
        let right_life = descriptor_entry_lifetime(steps, from, &root(2));
        joined_loan_authority(
            steps,
            start.clone(),
            &[(left, &left_life), (right, &right_life)],
        )
    };
    let found = choice(&steps).unwrap();
    assert_eq!(
        found.nodes[&found.root],
        LoanChoice::Arrivals(vec![
            LoanChoiceArrival {
                edge: Some(left_edge),
                source: (from, root(1))
            },
            LoanChoiceArrival {
                edge: Some(right_edge),
                source: (from, root(2))
            },
        ])
    );
    assert_eq!(found.nodes[&(from, root(1))], LoanChoice::Loan(left));
    assert_eq!(found.nodes[&(from, root(2))], LoanChoice::Loan(right));

    steps.get_mut(&from).unwrap().successors[1].bindings.clear();
    assert!(choice(&steps).is_none());
}

#[test]
fn a_loop_keeps_entry_authority_only_while_every_backedge_keeps_it() {
    let mut steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                successors: vec![edge(1)],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                inputs: vec![(Some(root(1)), None)],
                successors: vec![edge(2), edge(3)],
                ..Default::default()
            },
        ),
        (
            position(2),
            DescriptorFlowStep {
                successors: vec![edge(1)],
                ..Default::default()
            },
        ),
        (position(3), DescriptorFlowStep::default()),
        (
            position(4),
            DescriptorFlowStep {
                successors: vec![edge(5)],
                ..Default::default()
            },
        ),
        (
            position(5),
            DescriptorFlowStep {
                successors: vec![edge(4)],
                ..Default::default()
            },
        ),
    ]);
    let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
    for cut in [position(1), position(2)] {
        assert_eq!(lifetime.authorized[&cut], BTreeSet::from([root(1)]));
        assert_eq!(lifetime.surviving[&cut], BTreeSet::from([root(1)]));
    }
    assert!(lifetime.authorized[&position(4)].is_empty());
    assert!(lifetime.authorized[&position(5)].is_empty());
    steps
        .get_mut(&position(2))
        .unwrap()
        .definitions
        .push(root(1));
    let retired = descriptor_entry_lifetime(&steps, position(0), &root(1));
    assert!(retired.authorized[&position(1)].is_empty());
    assert_eq!(retired.surviving[&position(1)], BTreeSet::from([root(1)]));
}

#[test]
fn cleanup_effects_compose_in_order_while_edge_bindings_capture_before_cleanup() {
    let mut selected = edge(1);
    selected.edge = Some(semantic_vocabulary::EdgeId::new(1).unwrap());
    selected.definitions = vec![root(1), root(2)];
    selected.parameters = vec![root(4)];
    selected.bindings.push(
        DescriptorFlowBinding::new(
            DescriptorFlowSource::Location(root(2)),
            root(4),
            terminal_psi::StructuralAccess::Owned,
            terminal_psi::StructuralAccess::Owned,
        )
        .unwrap(),
    );
    for (ordinal, source, destination) in [(0, 1, 2), (2, 2, 3)] {
        selected.effects.push((
            terminal_psi::InvocationOwner::Cleanup {
                edge: selected.edge.unwrap(),
                action_ordinal: ordinal,
            },
            DescriptorFlowStep {
                definitions: vec![root(destination)],
                derivations: vec![(root(source), root(destination))],
                authority_derivations: vec![(root(source), root(destination))],
                inputs: vec![(Some(root(source)), None)],
                ..Default::default()
            },
        ));
    }
    assert_eq!(selected.forward(&root(1)), [root(3)]);
    assert_eq!(selected.forward_authority(&root(1)), [root(3)]);
    assert_eq!(selected.forward(&root(2)), [root(4)]);
    assert_eq!(selected.forward_authority(&root(2)), [root(4)]);
    selected.effects.reverse();
    assert!(selected.forward(&root(1)).is_empty());
}

#[test]
fn a_carrier_surviving_one_arrival_is_not_a_closed_resource() {
    let mut steps = BTreeMap::new();
    steps.insert(
        position(0),
        DescriptorFlowStep {
            successors: vec![edge(1), edge(2)],
            ..Default::default()
        },
    );
    steps.insert(
        position(1),
        DescriptorFlowStep {
            definitions: vec![root(1)],
            successors: vec![edge(3)],
            ..Default::default()
        },
    );
    steps.insert(
        position(2),
        DescriptorFlowStep {
            successors: vec![edge(3)],
            ..Default::default()
        },
    );
    steps.insert(position(3), DescriptorFlowStep::default());
    let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
    assert!(lifetime.authorized[&position(3)].is_empty());
    assert_eq!(lifetime.surviving[&position(3)], BTreeSet::from([root(1)]));
}

#[test]
fn a_local_or_edge_borrow_can_survive_only_one_branch() {
    let local = root(1);
    let parameter = root(2);
    let mut transfer = edge(1);
    transfer.parameters.push(parameter.clone());
    transfer.bindings.push(
        DescriptorFlowBinding::new(
            local.clone(),
            parameter.clone(),
            terminal_psi::StructuralAccess::SharedBorrow,
            terminal_psi::StructuralAccess::SharedBorrow,
        )
        .unwrap(),
    );
    let mut steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                definitions: vec![local.clone()],
                successors: vec![transfer.clone()],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                successors: vec![edge(2), edge(3)],
                ..Default::default()
            },
        ),
        (
            position(2),
            DescriptorFlowStep {
                definitions: vec![local.clone(), parameter.clone()],
                successors: vec![edge(4)],
                ..Default::default()
            },
        ),
        (
            position(3),
            DescriptorFlowStep {
                successors: vec![edge(4)],
                ..Default::default()
            },
        ),
        (position(4), DescriptorFlowStep::default()),
    ]);
    let formed = descriptor_location_lifetime(&steps, position(0), &local);
    let borrowed =
        descriptor_successor_lifetime(&steps, position(0), position(1), &parameter).unwrap();
    for (lifetime, location) in [(formed, &local), (borrowed, &parameter)] {
        assert!(lifetime.authorized[&position(1)].contains(location));
        assert!(lifetime.surviving[&position(4)].contains(location));
        assert!(lifetime.authorized[&position(4)].is_empty());
    }
    let birth = steps.get_mut(&position(0)).unwrap();
    birth.definitions.clear();
    birth.successors.push(transfer);
    let unformed = descriptor_location_lifetime(&steps, position(0), &local);
    assert!(unformed.authorized.is_empty());
    assert!(unformed.surviving.is_empty());
    assert!(descriptor_successor_lifetime(&steps, position(0), position(1), &parameter).is_none());
}

#[test]
fn a_selected_edge_captures_swapped_carriers_before_consuming_old_names() {
    let mut transfer = edge(1);
    transfer.parameters = vec![root(1), root(2)];
    transfer.definitions = vec![root(1), root(2)];
    transfer.bindings = vec![
        DescriptorFlowBinding::new(
            DescriptorFlowSource::Location(root(1)),
            root(2),
            terminal_psi::StructuralAccess::Owned,
            terminal_psi::StructuralAccess::Owned,
        )
        .unwrap(),
        DescriptorFlowBinding::new(
            DescriptorFlowSource::Location(root(2)),
            root(1),
            terminal_psi::StructuralAccess::Owned,
            terminal_psi::StructuralAccess::Owned,
        )
        .unwrap(),
    ];
    let steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                successors: vec![transfer],
                ..Default::default()
            },
        ),
        (position(1), DescriptorFlowStep::default()),
    ]);
    let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
    assert_eq!(lifetime.authorized[&position(1)], BTreeSet::from([root(2)]));
    assert_eq!(lifetime.surviving[&position(1)], BTreeSet::from([root(2)]));
}

#[test]
fn replacing_a_descriptor_keeps_its_write_without_observing_old_contents() {
    let mut replacement = DescriptorFlowStep {
        inputs: vec![(
            Some(root(1)),
            Some((
                access_vocabulary::AccessOperation::Write,
                DescriptorFlowSource::Unavailable(root(1).root),
            )),
        )],
        definitions: vec![root(1)],
        successors: vec![edge(1)],
        ..Default::default()
    };
    replacement.replace_value_inputs([root(2)]);
    assert_eq!(
        replacement.observed_inputs().collect::<Vec<_>>(),
        vec![&root(2)]
    );
    assert!(matches!(&replacement.inputs[0],
        (None, Some((access_vocabulary::AccessOperation::Write, DescriptorFlowSource::Unavailable(old))))
            if *old == root(1).root));
    let steps = BTreeMap::from([
        (position(0), replacement),
        (position(1), DescriptorFlowStep::default()),
    ]);
    let overwritten = descriptor_entry_lifetime(&steps, position(0), &root(1));
    let incoming = descriptor_entry_lifetime(&steps, position(0), &root(2));
    assert!(
        !overwritten.live.contains(&position(0)),
        "replacing the descriptor is not an observation of its old referent"
    );
    assert!(
        incoming.live.contains(&position(0)),
        "the incoming descriptor remains live through the replacement"
    );
}

#[test]
fn an_incoming_resource_observes_the_first_operation_before_transport() {
    let steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                inputs: vec![(Some(root(1)), None)],
                successors: vec![edge(1)],
                ..Default::default()
            },
        ),
        (position(1), DescriptorFlowStep::default()),
    ]);
    let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
    assert!(lifetime.live.contains(&position(0)));
    assert!(!lifetime.live.contains(&position(1)));
}

#[test]
fn indexed_sparse_lifetimes_match_dense_counterparts_without_empty_positions() {
    let local = root(1);
    let parameter = root(2);
    let mut transfer = edge(1);
    transfer.parameters.push(parameter.clone());
    transfer.bindings.push(
        DescriptorFlowBinding::new(
            local.clone(),
            parameter.clone(),
            terminal_psi::StructuralAccess::SharedBorrow,
            terminal_psi::StructuralAccess::SharedBorrow,
        )
        .unwrap(),
    );
    let steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                definitions: vec![local.clone()],
                successors: vec![transfer],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                successors: vec![edge(2), edge(3)],
                ..Default::default()
            },
        ),
        (
            position(2),
            DescriptorFlowStep {
                definitions: vec![local.clone(), parameter.clone()],
                successors: vec![edge(4)],
                ..Default::default()
            },
        ),
        (
            position(3),
            DescriptorFlowStep {
                successors: vec![edge(4)],
                ..Default::default()
            },
        ),
        (position(4), DescriptorFlowStep::default()),
    ]);
    let predecessors = FlowPredecessors::new(&steps);
    let dense_formed = descriptor_location_lifetime(&steps, position(0), &local);
    let sparse_formed =
        descriptor_location_lifetime_indexed(&steps, &predecessors, None, position(0), &local);
    let dense_borrowed =
        descriptor_successor_lifetime(&steps, position(0), position(1), &parameter).unwrap();
    let sparse_borrowed = descriptor_successor_lifetime_indexed(
        &steps,
        &predecessors,
        None,
        position(0),
        position(1),
        &parameter,
    )
    .unwrap();
    for (dense, sparse) in [
        (dense_formed, sparse_formed),
        (dense_borrowed, sparse_borrowed),
    ] {
        assert_eq!(sparse.live, dense.live);
        assert_eq!(sparse.demanded, dense.demanded);
        assert!(sparse.authorized.values().all(|names| !names.is_empty()));
        assert!(sparse.surviving.values().all(|names| !names.is_empty()));
        let dense_authorized: BTreeMap<_, _> = dense
            .authorized
            .into_iter()
            .filter(|(_, names)| !names.is_empty())
            .collect();
        let dense_surviving: BTreeMap<_, _> = dense
            .surviving
            .into_iter()
            .filter(|(_, names)| !names.is_empty())
            .collect();
        assert_eq!(sparse.authorized, dense_authorized);
        assert_eq!(sparse.surviving, dense_surviving);
    }
    assert!(
        descriptor_successor_lifetime_indexed(
            &steps,
            &predecessors,
            None,
            position(1),
            position(2),
            &parameter,
        )
        .is_none()
    );
}

#[test]
fn an_indexed_borrow_demands_its_view_through_the_headers_last_use() {
    use crate::storage::borrowed_storage::Projection;
    use semantic_vocabulary::ValueId;
    use terminal_psi::{StructuralAccess, StructuralPathSegment};

    let view = root(1);
    let header = root(2);
    let mut arrival = edge(1);
    arrival.parameters.push(header.clone());
    let mut binding = DescriptorFlowBinding::new(
        DescriptorFlowSource::Contents(DescriptorContentsQuery {
            root: view.root,
            steps: vec![DescriptorContentsStep::any_element(
                semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            )],
        }),
        header.clone(),
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
    )
    .unwrap();
    let index = Projection::new(&[StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(1).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    }]);
    binding.captured_storage = Some((view.clone(), index.clone()));
    assert!(binding.indexed_borrow_source().is_none());
    binding.captured_storage = Some((view.clone(), index.at(position(0))));
    binding.source_access = StructuralAccess::SharedBorrow;
    assert!(binding.indexed_borrow_source().is_none());
    binding.source_access = StructuralAccess::MutableBorrow;
    arrival.bindings.push(binding);
    let mut steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                inputs: vec![(Some(view.clone()), None)],
                successors: vec![arrival],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                inputs: vec![(Some(header.clone()), None)],
                successors: vec![edge(2)],
                ..Default::default()
            },
        ),
        (position(2), DescriptorFlowStep::default()),
    ]);
    let life = descriptor_entry_lifetime(&steps, position(0), &view);
    assert_eq!(life.live, BTreeSet::from([position(0), position(1)]));
    assert!(life.demanded[&position(1)].contains(&header));
    assert!(!life.authorized[&position(1)].contains(&header));

    // An actual escape from the header still reaches every continuation.
    steps.get_mut(&position(1)).unwrap().may_escape = true;
    assert!(
        descriptor_entry_lifetime(&steps, position(0), &view)
            .live
            .contains(&position(2))
    );
}

#[test]
fn a_loop_can_hand_its_last_view_to_the_next_reborrow() {
    let source = root(1);
    let result = root(2);
    let mut back = edge(0);
    back.parameters.push(source.clone());
    back.bindings.push(
        DescriptorFlowBinding::new(
            result.clone(),
            source.clone(),
            terminal_psi::StructuralAccess::MutableBorrow,
            terminal_psi::StructuralAccess::MutableBorrow,
        )
        .unwrap(),
    );
    let mut steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                definitions: vec![result.clone()],
                inputs: vec![(Some(source.clone()), None)],
                derivations: vec![(source.clone(), result.clone())],
                reborrows_view: true,
                successors: vec![edge(1)],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                inputs: vec![(Some(result.clone()), None)],
                successors: vec![back],
                ..Default::default()
            },
        ),
    ]);
    let lifetime = |steps: &_| descriptor_location_lifetime(steps, position(0), &result);
    let handed = lifetime(&steps);
    assert!(handed.live.contains(&position(0)));
    assert!(handed.reborrows_previous);
    assert!(handed.authorized[&position(1)].contains(&result));

    // The predecessor's source stays independently usable after the next
    // view is formed. A single static loan cannot stand for both.
    steps
        .get_mut(&position(1))
        .unwrap()
        .inputs
        .push((Some(source.clone()), None));
    assert!(!lifetime(&steps).reborrows_previous);
    steps.get_mut(&position(1)).unwrap().inputs.pop();

    steps.get_mut(&position(1)).unwrap().may_escape = true;
    assert!(!lifetime(&steps).reborrows_previous);
    steps.get_mut(&position(1)).unwrap().may_escape = false;
    steps.get_mut(&position(0)).unwrap().reborrows_view = false;
    assert!(!lifetime(&steps).reborrows_previous);
    steps.get_mut(&position(0)).unwrap().reborrows_view = true;

    // One return path narrows through a separate child loan. Its bytes
    // originate in the same buffer, but the next pass must name that child.
    let mut returning_child = edge(0);
    returning_child.parameters.push(source.clone());
    returning_child.bindings.push(
        DescriptorFlowBinding::new(
            root(3),
            source,
            terminal_psi::StructuralAccess::MutableBorrow,
            terminal_psi::StructuralAccess::MutableBorrow,
        )
        .unwrap(),
    );
    steps
        .get_mut(&position(1))
        .unwrap()
        .successors
        .push(edge(2));
    steps.insert(
        position(2),
        DescriptorFlowStep {
            definitions: vec![root(3)],
            inputs: vec![(Some(result.clone()), None)],
            derivations: vec![(result.clone(), root(3))],
            reborrows_view: true,
            successors: vec![returning_child],
            ..Default::default()
        },
    );
    assert!(!lifetime(&steps).reborrows_previous);
}
