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
