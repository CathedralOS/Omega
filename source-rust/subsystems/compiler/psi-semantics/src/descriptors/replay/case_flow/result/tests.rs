use super::*;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep,
};
use semantic_vocabulary::{BlockId, OperationId};
use terminal_psi::{StructuralCaseDeclaration, TerminalIdentityIndex};

fn position(index: u32) -> DescriptorPosition {
    (BlockId::new(1).unwrap(), index)
}
fn place(index: u64) -> DescriptorLocation {
    DescriptorLocation::whole(PlaceId::new(index).unwrap())
}
fn machine(id: u64, root: u64, successors: &[Vec<u32>]) -> DescriptorMachine {
    DescriptorMachine {
        id: MachineId::new(id).unwrap(),
        returns: None,
        places: vec![DescriptorPlace {
            producer: None,
            place: place(root).root,
            definition: DescriptorDefinition::Local,
        }],
        place_types: BTreeMap::from([(
            place(root).root,
            StructuralTypeId::new(1).unwrap(),
        )]),
        entry: position(0),
        flow: successors
            .iter()
            .enumerate()
            .map(|(index, targets)| {
                (
                    position(index as u32),
                    DescriptorFlowStep {
                        successors: targets
                            .iter()
                            .map(|target| DescriptorFlowEdge {
                                effects: Vec::new(),
                                edge: None,
                                definitions: Vec::new(),
                                target: position(*target),
                                parameters: vec![],
                                bindings: vec![],
                            })
                            .collect(),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: None,
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: vec![position((successors.len() - 1) as u32)],
    }
}

#[test]
fn indexed_case_subjects_validate_types_without_granting_cell_facts() {
    use crate::descriptor_lifetimes::{
        DescriptorContentsQuery, DescriptorContentsStep,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, ScalarTerm, ScalarType, ValueId,
    };

    let selected = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let declaration = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: vec![],
        id: selected,
        identity: TerminalIdentityIndex(1),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        shape: StructuralTypeShape::Sum {
            cases: (1..=2)
                .map(|id| StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: StructuralCaseId::new(id).unwrap(),
                    identity: TerminalIdentityIndex(id as u32),
                    fields: vec![],
                })
                .collect(),
        },
    };
    let mut collection = declaration.clone();
    collection.id = array;
    collection.identity = TerminalIdentityIndex(2);
    collection.shape = StructuralTypeShape::FixedArray {
        element: selected,
        length: terminal_psi::ArrayExtent::Literal(2),
    };
    let types = vec![declaration, collection];
    let mut body = machine(1, 1, &[vec![1], vec![]]);
    body.place_types.insert(place(1).root, array);
    let predicate = DescriptorCasePredicate {
        subject: DescriptorFlowSource::Contents(DescriptorContentsQuery {
            root: place(1).root,
            steps: vec![DescriptorContentsStep::Element {
                element: selected,
                selector: Some(Box::new(ScalarTerm::value(
                    ValueId::new(1).unwrap(),
                    ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                ))),
            }],
        }),
        case: StructuralCaseId::new(1).unwrap(),
    };
    body.case_successors
        .insert(position(0), vec![(predicate.clone(), true)]);
    let graph = vec![body];
    let replay = Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
    assert!(validate_graph(&replay, &graph[0]).is_some());
    let mut state = State::default();
    let before = state.cases.clone();
    for truth in [false, true] {
        assert_eq!(
            select(&replay, &graph[0], &mut state, &predicate, truth),
            Some(true)
        );
        assert!(
            state.cases == before,
            "one selected cell cannot narrow every possible cell"
        );
    }
    assert_eq!(
        membership(&replay, &graph[0], &state, &predicate),
        FlowFeasibility::Unknown
    );
    assert!(!established(&replay, &graph[0], &state, &predicate));
    assert!(establish(&replay, &graph[0], &mut state, &predicate).is_none());

    let mut forged = predicate.clone();
    let DescriptorFlowSource::Contents(query) = &mut forged.subject else {
        unreachable!()
    };
    let DescriptorContentsStep::Element { element, .. } = &mut query.steps[0] else {
        unreachable!()
    };
    *element = array;
    assert!(valid_subject(&replay, &graph[0], &forged).is_none());
    assert_eq!(select(&replay, &graph[0], &mut state, &forged, true), None);
    let mut malformed = vec![machine(1, 1, &[vec![1], vec![]])];
    malformed[0].place_types.insert(place(1).root, array);
    malformed[0]
        .case_successors
        .insert(position(0), vec![(forged.clone(), true)]);
    let receiver =
        Replay::with_slot_kind(&types, &malformed, DescriptorSlotKind::References);
    assert!(validate_graph(&receiver, &malformed[0]).is_none());
    let mut forged = predicate.clone();
    forged.case = StructuralCaseId::new(99).unwrap();
    assert!(valid_subject(&replay, &graph[0], &forged).is_none());
    malformed[0]
        .case_successors
        .insert(position(0), vec![(forged.clone(), true)]);
    let receiver =
        Replay::with_slot_kind(&types, &malformed, DescriptorSlotKind::References);
    assert!(validate_graph(&receiver, &malformed[0]).is_none());
    forged.case = predicate.case;
    forged.subject = DescriptorFlowSource::Unavailable(place(1).root);
    assert!(valid_subject(&replay, &graph[0], &forged).is_none());
    assert!(state.cases == before);
}

#[test]
fn completed_return_tags_union_all_exits_and_unknown_or_public_returns_grant_nothing() {
    let types = vec![StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        id: StructuralTypeId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        scalar_restriction: None,
        shape: StructuralTypeShape::Sum {
            cases: (1..=3)
                .map(|id| StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: StructuralCaseId::new(id).unwrap(),
                    identity: TerminalIdentityIndex(id as u32),
                    fields: vec![],
                })
                .collect(),
        },
    }];
    let mut caller = machine(1, 1, &[vec![1], vec![]]);
    caller
        .flow
        .get_mut(&position(0))
        .unwrap()
        .definitions
        .push(place(1));
    caller
        .insert_call(
            position(0),
            DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner: DescriptorInvocationOwner::Operation(
                    OperationId::new(1).unwrap(),
                ),
                target: DescriptorInvocationTarget::Machine(MachineId::new(2).unwrap()),
                arguments: vec![],
                argument_contents: vec![],
                result: Some(place(1)),
            },
        )
        .unwrap();
    let mut callee = machine(2, 2, &[vec![1, 3], vec![2], vec![], vec![4], vec![]]);
    callee.returns = Some(vec![(position(2), place(2)), (position(4), place(2))]);
    callee.normal_exits = vec![position(2), position(4)];
    for (point, case) in [(1, 1), (3, 2)] {
        callee.case_constructors.insert(
            position(point),
            vec![DescriptorCasePredicate {
                subject: place(2).into(),
                case: StructuralCaseId::new(case).unwrap(),
            }],
        );
    }
    let mut graph = vec![caller, callee];
    let predicate = |case| DescriptorCasePredicate {
        subject: place(1).into(),
        case: StructuralCaseId::new(case).unwrap(),
    };
    {
        let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
        assert_eq!(
            analysis.case_feasibility(graph[0].id, position(1), &predicate(1)),
            FlowFeasibility::Possible
        );
        assert_eq!(
            analysis.case_feasibility(graph[0].id, position(1), &predicate(2)),
            FlowFeasibility::Possible
        );
        assert_eq!(
            analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
            FlowFeasibility::ProvedImpossible
        );
        assert!(!analysis.case_is_established(graph[0].id, position(1), &predicate(1)));
    }
    graph[1].case_constructors.remove(&position(3));
    {
        let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
        assert_eq!(
            analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
            FlowFeasibility::Unknown
        );
    }
    graph[1].case_constructors.insert(
        position(3),
        vec![DescriptorCasePredicate {
            subject: place(2).into(),
            case: StructuralCaseId::new(1).unwrap(),
        }],
    );
    graph[0]
        .calls
        .get_mut(&position(0))
        .unwrap()
        .as_mut()
        .unwrap()[0]
        .target = DescriptorInvocationTarget::Public {
        effects_machine: Some(graph[1].id),
        realizations: vec![],
        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        results: vec![],
        contents: Vec::new(),
        invalidates_contents: false,
    };
    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
    assert_eq!(
        analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
        FlowFeasibility::Unknown
    );
}
