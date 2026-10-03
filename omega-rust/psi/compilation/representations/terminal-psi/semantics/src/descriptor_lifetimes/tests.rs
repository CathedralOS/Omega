use super::*;

fn position(index: u32) -> DescriptorPosition {
    (BlockId::new(1).unwrap(), index)
}
fn place(identity: u64) -> DescriptorLocation {
    DescriptorLocation::whole(PlaceId::new(identity).unwrap())
}
fn edge(target: u32) -> DescriptorFlowEdge {
    DescriptorFlowEdge {
        target: position(target),
        parameters: Vec::new(),
        bindings: Vec::new(),
    }
}

#[test]
fn authorization_follows_exact_forwarding_and_transparent_children_only() {
    let mut forward = edge(1);
    forward.parameters = vec![place(2)];
    forward.bindings = vec![(place(1), place(2))];
    let steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                definitions: vec![place(1)],
                successors: vec![forward],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                definitions: vec![place(3), place(4)],
                derivations: vec![(place(2), place(3)), (place(2), place(4))],
                // Place 3 is a transparent reborrow; place 4 captures that loan
                // into a record and cannot lend authority for the whole record.
                authority_derivations: vec![(place(2), place(3))],
                successors: vec![edge(2)],
                ..Default::default()
            },
        ),
        (
            position(2),
            DescriptorFlowStep {
                definitions: vec![place(2)],
                successors: vec![edge(3)],
                ..Default::default()
            },
        ),
        (position(3), DescriptorFlowStep::default()),
    ]);
    let authorized = descriptor_location_authorized_locations(&steps, position(0), &place(1));
    assert!(authorized[&position(1)].contains(&place(2)));
    assert!(authorized[&position(2)].contains(&place(3)));
    assert!(!authorized[&position(2)].contains(&place(4)));
    assert!(!authorized[&position(3)].contains(&place(2)));
    assert!(authorized[&position(3)].contains(&place(3)));
}

#[test]
fn authorization_rejects_a_forwarded_name_with_a_mixed_origin_arrival() {
    let mut good = edge(3);
    good.parameters = vec![place(4)];
    good.bindings = vec![(place(1), place(4))];
    let mut unrelated = edge(3);
    unrelated.parameters = vec![place(4)];
    unrelated.bindings = vec![(place(99), place(4))];
    let steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                definitions: vec![place(1)],
                successors: vec![edge(1), edge(2)],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                successors: vec![good],
                ..Default::default()
            },
        ),
        (
            position(2),
            DescriptorFlowStep {
                successors: vec![unrelated],
                ..Default::default()
            },
        ),
        (position(3), DescriptorFlowStep::default()),
    ]);
    let authorized = descriptor_location_authorized_locations(&steps, position(0), &place(1));
    assert!(authorized[&position(3)].contains(&place(1)));
    assert!(!authorized[&position(3)].contains(&place(4)));
}

#[test]
fn authorization_does_not_identify_a_repeated_static_birth_as_one_loan() {
    let steps = BTreeMap::from([
        (
            position(0),
            DescriptorFlowStep {
                definitions: vec![place(1)],
                successors: vec![edge(1)],
                ..Default::default()
            },
        ),
        (
            position(1),
            DescriptorFlowStep {
                inputs: vec![place(1)],
                successors: vec![edge(0)],
                ..Default::default()
            },
        ),
    ]);
    assert!(descriptor_location_authorized_locations(&steps, position(0), &place(1)).is_empty());
}
