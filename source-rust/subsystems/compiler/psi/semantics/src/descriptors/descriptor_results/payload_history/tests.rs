use super::*;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource,
    DescriptorPathSegment,
};
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
};

fn position(block: u64) -> DescriptorPosition {
    (BlockId::new(block).unwrap(), 0)
}

fn condition(value: u64) -> Proposition {
    Proposition::Equal(
        ScalarTerm::Boolean(true),
        ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
    )
}

fn forked_history(branches: &[u64]) -> (DescriptorMachine, PayloadMembershipContract) {
    let root = PlaceId::new(1).unwrap();
    let carrier = StructuralTypeId::new(1).unwrap();
    let step = |targets: &[u64]| DescriptorFlowStep {
        successors: targets
            .iter()
            .map(|target| DescriptorFlowEdge {
                edge: None,
                effects: Vec::new(),
                definitions: Vec::new(),
                target: position(*target),
                parameters: Vec::new(),
                bindings: Vec::new(),
            })
            .collect(),
        ..Default::default()
    };
    let machine = DescriptorMachine {
        id: MachineId::new(1).unwrap(),
        returns: None,
        places: vec![DescriptorPlace {
            producer: None,
            place: root,
            definition: DescriptorDefinition::Parameter {
                position: 0,
                access: StructuralAccess::Owned,
                structural_type: carrier,
            },
        }],
        place_types: BTreeMap::from([(root, carrier)]),
        entry: position(1),
        flow: BTreeMap::from([
            (position(1), step(branches)),
            (position(2), step(&[4])),
            (position(3), step(&[4])),
            (position(4), step(&[])),
        ]),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: Some((
            semantic_vocabulary::PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Boolean),
                (ValueId::new(2).unwrap(), ScalarType::Boolean),
            ])
            .unwrap(),
            vec![condition(2)],
        )),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::from([(
            position(2),
            vec![condition(1)],
        )]),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: vec![position(4)],
    };
    let contract = PayloadMembershipContract {
        carrier,
        scalar_requirements: vec![condition(1)],
        scalar_observations: Vec::new(),
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    (machine, contract)
}

#[test]
fn shared_history_refuses_another_inventory_or_same_coordinate_graph() {
    let (machine, contract) = forked_history(&[2, 3]);
    let identity = machine.id;
    let entry = machine.entry;
    let graph = [machine];
    let root = PlaceId::new(1).unwrap();
    let contracts = [(root, contract)];
    let mut history = PayloadMembershipHistory::for_current_contracts(
        &[],
        &[],
        &graph,
        identity,
        &[],
        &contracts,
    )
    .unwrap();
    let mut views = DescriptorAnalysis::new(&[], &graph);
    history
        .current_payload_contract_in(&mut views, entry, root)
        .unwrap();
    assert!(history.replay.cached.is_some());

    let mut borrowed = DescriptorAnalysis::with_borrowed_storage_slots(&[], &graph);
    assert_eq!(
        history
            .current_payload_contract_in(&mut borrowed, entry, root)
            .err(),
        Some("payload history analysis changed its descriptor inventory")
    );
    let other_graph = [forked_history(&[2, 3]).0];
    let mut same_coordinates = DescriptorAnalysis::new(&[], &other_graph);
    assert_eq!(
        history
            .current_payload_contract_in(&mut same_coordinates, entry, root)
            .err(),
        Some("payload history analysis belongs to another immutable source graph")
    );
    assert!(history.replay.cached.is_some());
    history
        .current_payload_contract_in(&mut views, entry, root)
        .unwrap();
    assert_eq!(history.replay.inventory, Some(DescriptorSlotKind::Views));
}

#[test]
fn entry_observation_does_not_inherit_an_unpublished_ambient_premise() {
    let (machine, source) = forked_history(&[2, 3]);
    let observed = current_payload_contract(
        &[],
        &[],
        std::slice::from_ref(&machine),
        machine.id,
        machine.entry,
        PlaceId::new(1).unwrap(),
        &|_| Some(source.clone()),
    )
    .unwrap();
    assert_eq!(observed.scalar_requirements, vec![condition(1)]);
}

#[test]
fn joined_payload_keeps_both_paths_facts_but_not_a_retired_instance() {
    for branches in [[2, 3], [3, 2]] {
        for retire in [false, true] {
            let (mut machine, source) = forked_history(&branches);
            if retire {
                machine
                    .payload_scalar_definitions
                    .insert(position(3), vec![ValueId::new(1).unwrap()]);
            }
            let observed = current_payload_contract(
                &[],
                &[],
                std::slice::from_ref(&machine),
                machine.id,
                position(4),
                PlaceId::new(1).unwrap(),
                &|_| Some(source.clone()),
            )
            .unwrap();
            let mut expected = vec![condition(2)];
            if !retire {
                expected.push(condition(1));
            }
            expected.sort();
            assert_eq!(observed.scalar_requirements, expected);
        }
    }
}

#[test]
fn predicate_edge_captures_both_arrival_routes_before_instance_retirement() {
    use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

    let root = PlaceId::new(1).unwrap();
    let selected = PlaceId::new(2).unwrap();
    let predicate = Proposition::Conjunction(vec![condition(3), condition(4)]);
    for retire in [false, true] {
        let (mut machine, source) = forked_history(&[2]);
        let _ = machine.flow.remove(&position(3));
        let types = [StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            id: source.carrier,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            descriptor_cleanup: true,
            nominal_cleanup: None,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            scalar_restriction: None,
        }];
        machine.places.push(DescriptorPlace {
            producer: None,
            place: selected,
            definition: DescriptorDefinition::Local,
        });
        machine.place_types.insert(selected, source.carrier);
        let edge = &mut machine.flow.get_mut(&position(2)).unwrap().successors[0];
        edge.parameters = vec![
            DescriptorLocation::whole(root),
            DescriptorLocation::whole(selected),
        ];
        edge.bindings = vec![
            DescriptorFlowBinding::new(
                DescriptorLocation::whole(root),
                DescriptorLocation::whole(root),
                StructuralAccess::Owned,
                StructuralAccess::Owned,
            )
            .unwrap(),
            DescriptorFlowBinding::new(
                DescriptorFlowSource::Contents(DescriptorContentsQuery {
                    root,
                    steps: Vec::new(),
                }),
                DescriptorLocation::whole(selected),
                StructuralAccess::Owned,
                StructuralAccess::Owned,
            )
            .unwrap(),
        ];
        machine
            .payload_edge_predicates
            .insert(position(2), vec![Some(predicate.clone())]);
        if retire {
            machine
                .payload_scalar_parameters
                .insert(position(4).0, vec![ValueId::new(3).unwrap()]);
        }
        let mut expected = vec![condition(1), condition(2), condition(4)];
        if !retire {
            expected.extend([condition(3), predicate.clone()]);
        }
        expected.sort();
        for payload in [root, selected] {
            let observed = current_payload_contract(
                &types,
                &[],
                std::slice::from_ref(&machine),
                machine.id,
                position(4),
                payload,
                &|_| Some(source.clone()),
            )
            .unwrap();
            assert_eq!(observed.scalar_requirements, expected);
        }
    }
}

#[test]
fn published_payload_retains_capacity_and_its_complete_theory() {
    let root = PlaceId::new(1).unwrap();
    let carrier = StructuralTypeId::new(1).unwrap();
    let mut requirements = Vec::with_capacity(32);
    requirements.extend([condition(1), condition(2)]);
    requirements.sort();
    let retained_capacity = requirements.capacity();
    let contract = PayloadMembershipContract {
        carrier,
        scalar_observations: Vec::new(),
        scalar_requirements: requirements,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut state = State {
        values: PayloadContracts::from([(root, contract)]),
        scalar_requirements: Arc::new(vec![SharedPremise::new(condition(1))]),
        ..Default::default()
    };
    let before = state.contract(root, None).unwrap();
    state.publish_requirements();
    assert_eq!(state.contract(root, None).unwrap(), before);
    let local = &state.values[&root].scalar_requirements;
    assert_eq!(local, &[condition(2)]);
    assert_eq!(local.capacity(), retained_capacity);
    state.publish_requirements();
    assert_eq!(state.contract(root, None).unwrap(), before);
    assert_eq!(
        state.values[&root].scalar_requirements.capacity(),
        retained_capacity
    );
}

#[test]
fn equal_raw_observations_still_normalize_at_meet() {
    let root = PlaceId::new(1).unwrap();
    let observed = super::super::PayloadScalarObservation {
        path: Vec::new(),
        value: Some(ScalarTerm::Boolean(true)),
        entry: None,
    };
    for observations in [
        vec![observed.clone(), observed],
        vec![super::super::PayloadScalarObservation {
            path: Vec::new(),
            value: None,
            entry: None,
        }],
    ] {
        for different_ambient in [false, true] {
            let mut retained = State {
                values: PayloadContracts::from([(
                    root,
                    PayloadMembershipContract {
                        carrier: StructuralTypeId::new(1).unwrap(),
                        scalar_observations: observations.clone(),
                        scalar_requirements: Vec::new(),
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                )]),
                ..Default::default()
            };
            let mut arrival = retained.clone();
            if different_ambient {
                arrival.scalar_requirements =
                    Arc::new(vec![SharedPremise::new(condition(1))]);
            }
            let changed = retained.meet(arrival);
            assert!(
                retained
                    .values
                    .get(&root)
                    .unwrap()
                    .scalar_observations
                    .is_empty()
            );
            assert_eq!(changed, observations.len() == 2);
        }
    }
}

#[test]
fn untouched_payload_rows_stay_shared_and_changed_rows_detach() {
    let root = PlaceId::new(1).unwrap();
    let other = PlaceId::new(2).unwrap();
    let contract = PayloadMembershipContract {
        carrier: StructuralTypeId::new(1).unwrap(),
        scalar_observations: Vec::new(),
        scalar_requirements: vec![condition(1)],
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let original = State {
        values: PayloadContracts::from([(root, contract.clone()), (other, contract)]),
        ..Default::default()
    };
    let mut later = original.clone();
    later.publish_requirements();
    retire_scalar_instances(&mut later, &[ValueId::new(2).unwrap()]);
    clear(&mut later.values);
    assert!(!super::super::payload_predicates::closure_may_change(
        &[],
        &[],
        later.values.get(&root).unwrap()
    ));
    for root in [root, other] {
        assert!(Arc::ptr_eq(
            &original.values.0[&root],
            &later.values.0[&root]
        ));
    }
    later
        .values
        .get_mut(&root)
        .unwrap()
        .scalar_requirements
        .push(condition(2));
    assert!(!Arc::ptr_eq(
        &original.values.0[&root],
        &later.values.0[&root]
    ));
    assert!(Arc::ptr_eq(
        &original.values.0[&other],
        &later.values.0[&other]
    ));
    assert_eq!(
        original.values.get(&root).unwrap().scalar_requirements,
        vec![condition(1)]
    );
    assert_eq!(
        later.values.get(&root).unwrap().scalar_requirements,
        vec![condition(1), condition(2)]
    );
}

#[test]
fn publication_and_retirement_detach_only_affected_payload_rows() {
    let root = PlaceId::new(1).unwrap();
    let other = PlaceId::new(2).unwrap();
    let contract = |premise| PayloadMembershipContract {
        carrier: StructuralTypeId::new(1).unwrap(),
        scalar_observations: Vec::new(),
        scalar_requirements: vec![condition(premise)],
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let original = State {
        values: PayloadContracts::from([(root, contract(1)), (other, contract(2))]),
        ..Default::default()
    };
    let mut published = original.clone();
    published.scalar_requirements = Arc::new(vec![SharedPremise::new(condition(1))]);
    published.publish_requirements();
    assert!(!Arc::ptr_eq(
        &original.values.0[&root],
        &published.values.0[&root]
    ));
    assert!(Arc::ptr_eq(
        &original.values.0[&other],
        &published.values.0[&other]
    ));
    assert!(
        published
            .values
            .get(&root)
            .unwrap()
            .scalar_requirements
            .is_empty()
    );
    let mut retired = original.clone();
    retire_scalar_instances(&mut retired, &[ValueId::new(1).unwrap()]);
    assert!(!Arc::ptr_eq(
        &original.values.0[&root],
        &retired.values.0[&root]
    ));
    assert!(Arc::ptr_eq(
        &original.values.0[&other],
        &retired.values.0[&other]
    ));
    assert!(
        retired
            .values
            .get(&root)
            .unwrap()
            .scalar_requirements
            .is_empty()
    );
    assert_eq!(
        original.values.get(&root).unwrap().scalar_requirements,
        vec![condition(1)]
    );
}

// Only contents strictly inside an existing subject may change
// without replacing that subject's issuer-bound membership.
#[test]
fn content_writes_keep_only_existing_content_independent_memberships() {
    use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
    use terminal_psi::{
        StructuralDomainDeclaration, StructuralDomainPredicate,
        StructuralPathQualification,
    };
    let id = |raw| StructuralDomainId::new(raw).unwrap();
    let semantic = |raw| DomainSemanticId::new(raw).unwrap();
    let carrier = StructuralTypeId::new(1).unwrap();
    let domain = |raw| StructuralDomainDeclaration {
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: Some(StructuralDomainPredicate {
            subject: PlaceId::new(1).unwrap(),
            proposition: Proposition::Truth,
        }),
        id: id(raw),
        semantic_domain: semantic(raw),
        identity: Default::default(),
        subject_identity: Default::default(),
        carrier,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: Some(Vec::new()),
        access_policy: None,
    };
    let mut predicate = domain(2);
    predicate.predicate = Some(StructuralDomainPredicate {
        subject: PlaceId::new(1).unwrap(),
        proposition: Proposition::Falsehood,
    });
    let mut incomplete = domain(3);
    incomplete.complete_predicate_theory = false;
    let mut predicate_dependency = domain(4);
    predicate_dependency.required_memberships = Some(vec![semantic(2)]);
    let mut independent_dependency = domain(5);
    independent_dependency.required_memberships = Some(vec![semantic(1)]);
    let mut unknown = domain(6);
    unknown.required_memberships = None;
    let mut bytes = domain(7);
    bytes.byte_predicate = Some(terminal_psi::ByteSequencePredicate::AsciiOnly);
    let domains = [
        domain(1),
        predicate,
        incomplete,
        predicate_dependency,
        independent_dependency,
        unknown,
        bytes,
    ];
    let content_independent =
        StructuralDomainDeclaration::content_independent_memberships(&domains);
    assert_eq!(content_independent, [id(1), id(5)]);
    let mut malformed = domains.to_vec();
    let mut cycle = domain(8);
    cycle.required_memberships = Some(vec![semantic(8)]);
    let mut missing = domain(9);
    missing.required_memberships = Some(vec![semantic(99)]);
    let mut missing_predicate = domain(10);
    missing_predicate.predicate = None;
    malformed.extend([cycle, missing, missing_predicate]);
    assert_eq!(
        StructuralDomainDeclaration::content_independent_memberships(&malformed),
        content_independent
    );
    malformed.push(domain(1));
    assert!(
        StructuralDomainDeclaration::content_independent_memberships(&malformed)
            .is_empty()
    );
    let mut ambiguous = domains.to_vec();
    let mut duplicate_semantic = domain(10);
    duplicate_semantic.semantic_domain = semantic(1);
    ambiguous.push(duplicate_semantic);
    assert!(
        StructuralDomainDeclaration::content_independent_memberships(&ambiguous)
            .is_empty()
    );
    let first = StructuralPathSegment::FixedIndex(0);
    let second = StructuralPathSegment::FixedIndex(1);
    let original = PayloadMembershipContract {
        carrier,
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
        qualifications: (1..=7).map(id).collect(),
        projected_qualifications: (1..=7)
            .map(|raw| StructuralPathQualification {
                path: vec![first.clone()],
                domain: id(raw),
            })
            .chain([StructuralPathQualification {
                path: vec![second.clone()],
                domain: id(2),
            }])
            .collect(),
    };
    let mut changed = original.clone();
    retire(
        &content_independent,
        &mut changed,
        std::slice::from_ref(&first),
        &[DescriptorPathSegment::FixedIndex(0)],
    );
    assert_eq!(changed.qualifications, [id(1), id(5)]);
    assert_eq!(
        changed.projected_qualifications,
        [StructuralPathQualification {
            path: vec![second],
            domain: id(2)
        }],
        "an exact field replacement loses its tag; an unrelated sibling survives"
    );
    let mut nested = original.clone();
    retire(
        &content_independent,
        &mut nested,
        &[first.clone(), StructuralPathSegment::FixedIndex(0)],
        &[
            DescriptorPathSegment::FixedIndex(0),
            DescriptorPathSegment::FixedIndex(0),
        ],
    );
    assert_eq!(nested.qualifications, [id(1), id(5)]);
    assert_eq!(
        nested
            .projected_qualifications
            .iter()
            .map(|row| row.domain)
            .collect::<Vec<_>>(),
        [id(1), id(5), id(2)]
    );
    let mut replaced = original.clone();
    retire(&content_independent, &mut replaced, &[], &[]);
    assert!(
        replaced.qualifications.is_empty()
            && replaced.projected_qualifications.is_empty(),
        "whole replacement or moved-out slot reuse cannot reissue an old tag"
    );
    let root = PlaceId::new(1).unwrap();
    let mut unknown_write = PayloadContracts::from([(root, original)]);
    clear(&mut unknown_write);
    assert!(
        unknown_write.get(&root).unwrap().qualifications.is_empty(),
        "unknown effects retain no content-independent membership shortcut"
    );
    retire(
        &content_independent,
        &mut replaced,
        &[first],
        &[DescriptorPathSegment::FixedIndex(0)],
    );
    assert!(
        replaced.qualifications.is_empty(),
        "an unqualified new producer gains nothing"
    );
}

#[test]
fn retained_payload_snapshot_survives_later_writes_retirement_and_closure() {
    use semantic_vocabulary::{PropositionId, StructuralDomainId};

    let root = PlaceId::new(1).unwrap();
    let path = vec![DescriptorPathSegment::FixedIndex(0)];
    let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Boolean);
    let domain = |id| StructuralDomainId::new(id).unwrap();
    let mut requirements = vec![
        condition(1),
        Proposition::Atom(PropositionId::new(1).unwrap()),
        Proposition::Conjunction(vec![
            Proposition::Atom(PropositionId::new(1).unwrap()),
            condition(2),
        ]),
    ];
    requirements.sort();
    let shared = Arc::new(
        requirements
            .iter()
            .cloned()
            .map(SharedPremise::new)
            .collect(),
    );
    let original = PayloadMembershipContract {
        carrier: StructuralTypeId::new(1).unwrap(),
        scalar_observations: vec![super::super::PayloadScalarObservation {
            path: path.clone(),
            value: Some(value(1)),
            entry: None,
        }],
        scalar_requirements: requirements,
        qualifications: vec![domain(2), domain(1), domain(2)],
        projected_qualifications: Vec::new(),
    };
    let retained = State {
        values: PayloadContracts::from([(root, original.clone())]),
        scalar_requirements: Arc::clone(&shared),
        shared_requirements: shared,
    };
    enum Change {
        Store,
        UnknownWrite,
        RetireInstance,
        Close,
    }
    for change in [
        Change::Store,
        Change::UnknownWrite,
        Change::RetireInstance,
        Change::Close,
    ] {
        let mut later = retained.clone();
        let mut expected = original.clone();
        match change {
            Change::Store => {
                let contract = later.values.get_mut(&root).unwrap();
                retire(
                    &[],
                    contract,
                    &[StructuralPathSegment::FixedIndex(0)],
                    &path,
                );
                assert_eq!(observe_scalar(contract, &path, &value(2)), None);
                expected.scalar_observations[0].value = Some(value(2));
                expected.qualifications.clear();
            }
            Change::UnknownWrite => {
                clear(&mut later.values);
                expected.scalar_observations.clear();
                expected.qualifications.clear();
            }
            Change::RetireInstance => {
                retire_scalar_instances(&mut later, &[ValueId::new(1).unwrap()]);
                expected.scalar_observations.clear();
                expected.scalar_requirements.clear();
                assert!(later.shared_requirements.is_empty());
                assert_eq!(later.scalar_requirements.len(), 2);
                assert!(later.scalar_requirements.iter().all(|premise| {
                    !premise
                        .as_ref()
                        .any_value_id(|value| value == ValueId::new(1).unwrap())
                }));
            }
            Change::Close => {
                super::super::payload_predicates::close_with_shared_requirements::<
                    Proposition,
                >(
                    &[], &[], later.values.get_mut(&root).unwrap(), &[], None
                );
                expected.qualifications = vec![domain(1), domain(2)];
            }
        }
        assert!(!Arc::ptr_eq(
            &retained.values.0[&root],
            &later.values.0[&root]
        ));
        assert_eq!(retained.contract(root, None).unwrap(), original);
        assert_eq!(later.contract(root, None).unwrap(), expected);
    }
    let mut later = retained.clone();
    retire_scalar_instances(&mut later, &[ValueId::new(2).unwrap()]);
    assert_eq!(later.scalar_requirements.len(), 2);
    assert!(later.scalar_requirements.iter().all(|premise| {
        !premise
            .as_ref()
            .any_value_id(|value| value == ValueId::new(2).unwrap())
    }));
    assert_eq!(later.shared_requirements.len(), 1);
    assert_eq!(later.shared_requirements[0].as_ref(), &condition(1));
}

#[test]
fn membership_filter_preserves_equal_raw_rosters_and_filters_different_ones() {
    for original in [vec![], vec![1], vec![3, 1, 3, 2, 1]] {
        let mut retained = original.clone();
        retain_common_memberships(&mut retained, &original);
        assert_eq!(retained, original);
    }
    for (original, incoming) in [
        (vec![3, 1, 3, 2, 1], vec![2, 3, 2]),
        (vec![3, 1, 3, 2, 1], vec![1, 2, 3]),
        (vec![3, 1], vec![]),
        (vec![], vec![3, 1]),
        (vec![3, 1], vec![2]),
    ] {
        let expected: Vec<_> = original
            .iter()
            .copied()
            .filter(|value| incoming.contains(value))
            .collect();
        let mut retained = original;
        retain_common_memberships(&mut retained, &incoming);
        assert_eq!(retained, expected);
    }
}

#[test]
fn rereads_preserve_common_observation_but_writes_and_instances_retire_it() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = |identity| {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer),
        )
    };
    let path = vec![DescriptorPathSegment::FixedIndex(0)];
    let mut contract = PayloadMembershipContract {
        carrier: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    assert_eq!(observe_scalar(&mut contract, &path, &value(1)), None);
    let mut other_branch = contract.clone();
    assert_eq!(
        observe_scalar(&mut contract, &path, &value(2)),
        Some(Proposition::Equal(value(2), value(1)))
    );
    assert_eq!(
        observe_scalar(&mut other_branch, &path, &value(3)),
        Some(Proposition::Equal(value(3), value(1)))
    );
    assert_eq!(
        contract.scalar_observations,
        other_branch.scalar_observations
    );
    retire(
        &[],
        &mut contract,
        &[StructuralPathSegment::FixedIndex(0)],
        &path,
    );
    assert_eq!(observe_scalar(&mut contract, &path, &value(4)), None);
    assert_eq!(contract.scalar_observations[0].value, Some(value(4)));
    let root = PlaceId::new(1).unwrap();
    let mut state = State {
        values: PayloadContracts::from([(root, other_branch)]),
        scalar_requirements: Arc::default(),
        shared_requirements: Arc::default(),
    };
    retire_scalar_instances(&mut state, &[ValueId::new(1).unwrap()]);
    assert_eq!(
        observe_scalar(state.values.get_mut(&root).unwrap(), &path, &value(5)),
        None
    );
}
