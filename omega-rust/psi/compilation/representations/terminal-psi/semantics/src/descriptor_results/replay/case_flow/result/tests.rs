use super::*;
use crate::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorFlowStep};
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
            place: place(root).root,
            definition: DescriptorDefinition::Local,
        }],
        place_types: BTreeMap::from([(place(root).root, StructuralTypeId::new(1).unwrap())]),
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
fn completed_return_tags_union_all_exits_and_unknown_or_public_returns_grant_nothing() {
    let types = vec![StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape: StructuralTypeShape::Sum {
            cases: (1..=3)
                .map(|id| StructuralCaseDeclaration {
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
    caller.calls.insert(
        position(0),
        DescriptorInvocation {
            operation: OperationId::new(1).unwrap(),
            target: DescriptorInvocationTarget::Machine(MachineId::new(2).unwrap()),
            arguments: vec![],
            argument_contents: vec![],
            result: Some(place(1)),
        },
    );
    let mut callee = machine(2, 2, &[vec![1, 3], vec![2], vec![], vec![4], vec![]]);
    callee.returns = Some(vec![(position(2), place(2)), (position(4), place(2))]);
    callee.normal_exits = vec![position(2), position(4)];
    for (point, case) in [(1, 1), (3, 2)] {
        callee.case_constructors.insert(
            position(point),
            vec![DescriptorCasePredicate {
                subject: place(2),
                case: StructuralCaseId::new(case).unwrap(),
            }],
        );
    }
    let mut graph = vec![caller, callee];
    let predicate = |case| DescriptorCasePredicate {
        subject: place(1),
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
    // One unconstrained normal return invalidates the guarantee, even though
    // the other return still establishes its own positive constructor tag.
    graph[1].case_constructors.remove(&position(3));
    {
        let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
        assert_eq!(
            analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
            FlowFeasibility::Unknown
        );
    }
    // A public requirement may expose effects from this exact implementation,
    // but its private tag postcondition is not part of the public promise.
    graph[1].case_constructors.insert(
        position(3),
        vec![DescriptorCasePredicate {
            subject: place(2),
            case: StructuralCaseId::new(1).unwrap(),
        }],
    );
    graph[0].calls.get_mut(&position(0)).unwrap().target = DescriptorInvocationTarget::Public {
        effects_machine: Some(graph[1].id),
        realizations: vec![],
        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        results: vec![],
        invalidates_contents: false,
    };
    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
    assert_eq!(
        analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
        FlowFeasibility::Unknown
    );
}
