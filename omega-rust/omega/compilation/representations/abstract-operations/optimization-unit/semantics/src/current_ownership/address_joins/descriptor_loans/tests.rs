use super::*;
use semantic_vocabulary::{
    MachineId, ObligationId, OperationId, ScalarType, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};

fn position(node: u32) -> Position {
    (BlockId::new(1).unwrap(), node)
}

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

fn edge(node: u32) -> DescriptorFlowEdge {
    DescriptorFlowEdge {
        target: position(node),
        parameters: Vec::new(),
        bindings: Vec::new(),
    }
}

fn repeated_formation() -> DescriptorFlow {
    // write -> form view -> observe view -> backedge. The next write happens
    // before the new formation, after all observations of the previous value.
    DescriptorFlow {
        steps: BTreeMap::from([
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
                    definitions: vec![place(1)],
                    successors: vec![edge(2)],
                    ..Default::default()
                },
            ),
            (
                position(2),
                DescriptorFlowStep {
                    inputs: vec![place(1)],
                    successors: vec![edge(3)],
                    ..Default::default()
                },
            ),
            (
                position(3),
                DescriptorFlowStep {
                    successors: vec![edge(0)],
                    ..Default::default()
                },
            ),
        ]),
    }
}

#[test]
fn repeated_formation_retires_the_previous_unforwarded_instance() {
    let flow = repeated_formation();
    let live = flow.live_positions(position(1), place(1));
    assert!(live.contains(&position(2)));
    assert!(
        !live.contains(&position(0)),
        "next write has no surviving descriptor"
    );
    assert!(
        !live.contains(&position(1)),
        "redefinition does not observe the old value"
    );
}

#[test]
fn forwarded_old_alias_survives_reformation_of_the_original_name() {
    let mut flow = repeated_formation();
    // The backedge carries the old descriptor into a distinct parameter;
    // that alias is observed after the original descriptor is reformed.
    flow.steps.get_mut(&position(3)).unwrap().inputs = vec![place(1)];
    flow.steps.get_mut(&position(3)).unwrap().successors[0] = DescriptorFlowEdge {
        target: position(0),
        parameters: vec![place(2)],
        bindings: vec![(place(1), place(2))],
    };
    flow.steps
        .get_mut(&position(2))
        .unwrap()
        .inputs
        .push(place(2));
    let live = flow.live_positions(position(1), place(1));
    assert!(
        live.contains(&position(0)),
        "old forwarded alias still pins the next write"
    );
    assert!(
        live.contains(&position(1)),
        "reforming one name cannot retire its aliases"
    );
}

#[test]
fn simultaneous_arrival_reads_sources_before_overwriting_destinations() {
    let mut flow = repeated_formation();
    flow.steps.get_mut(&position(3)).unwrap().successors[0] = DescriptorFlowEdge {
        target: position(0),
        parameters: vec![place(1), place(2)],
        bindings: vec![(place(1), place(2)), (place(2), place(1))],
    };
    flow.steps
        .get_mut(&position(2))
        .unwrap()
        .inputs
        .push(place(2));
    assert!(
        flow.live_positions(position(1), place(1))
            .contains(&position(0))
    );
}

#[test]
fn overwritten_arrival_does_not_keep_a_previous_cohort_alive() {
    let mut flow = repeated_formation();
    flow.steps.get_mut(&position(3)).unwrap().successors[0] = DescriptorFlowEdge {
        target: position(0),
        parameters: vec![place(1)],
        bindings: vec![(place(9), place(1))],
    };
    assert!(
        !flow
            .live_positions(position(1), place(1))
            .contains(&position(0))
    );
}

#[test]
fn unknown_escape_survives_descriptor_reformation() {
    let mut flow = repeated_formation();
    flow.steps.get_mut(&position(2)).unwrap().may_escape = true;
    let live = flow.live_positions(position(1), place(1));
    assert!(live.contains(&position(0)));
    assert!(live.contains(&position(1)));
}

#[test]
fn replacement_reads_the_old_source_before_defining_its_result() {
    let mut flow = repeated_formation();
    let birth = flow.steps.get_mut(&position(1)).unwrap();
    birth.inputs = vec![place(1)];
    birth.derivation = Some((place(1), place(1)));
    let live = flow.live_positions(position(1), place(1));
    assert!(
        live.contains(&position(0)),
        "the next formation reads the preceding instance"
    );
    assert!(live.contains(&position(1)));
}

#[test]
fn derived_child_keeps_its_parent_cohort_after_the_parent_name_is_replaced() {
    let mut flow = repeated_formation();
    let child = flow.steps.get_mut(&position(3)).unwrap();
    child.inputs = vec![place(1)];
    child.definitions = vec![place(2)];
    child.derivation = Some((place(1), place(2)));
    flow.steps
        .get_mut(&position(2))
        .unwrap()
        .inputs
        .push(place(2));
    assert!(
        flow.live_positions(position(1), place(1))
            .contains(&position(0))
    );
}

fn instruction(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}

fn value(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}

fn scalar(operation: u64) -> O {
    O::BooleanConstant {
        psi_operation: instruction(operation),
        result: value(operation),
        value: false,
    }
}

// This is a receiver-local fixture, not a claim that the deliberately corrupted
// operations constitute a verified complete module. It exercises the actual
// operation/catalog adapter and DescriptorLoans builder, not hand-built flow.
fn receiver_fixture(intervening: O, observe_after_write: bool) -> PsiOptimizationFunction {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let result = terminal_psi::StructuralOperationResult {
        place: place(1),
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let operations = vec![
        O::ByteSequenceSubslice {
            psi_operation: instruction(1),
            result,
            source: place(9),
            start: value(1),
            end: value(2),
            length: value(3),
            obligation: ObligationId::new(1).unwrap(),
        },
        intervening,
        O::ByteSequenceWrite {
            psi_operation: instruction(3),
            destination: place(9),
            index: value(1),
            value: value(2),
            length: value(3),
            obligation: ObligationId::new(2).unwrap(),
        },
        if observe_after_write {
            O::ByteSequenceLength {
                psi_operation: instruction(4),
                source: place(1),
                result: abstract_operations::AbstractResult {
                    value: value(4),
                    scalar_type: ScalarType::Boolean,
                },
            }
        } else {
            scalar(4)
        },
    ];
    PsiOptimizationFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: position(0).0,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
            id: place(1),
            kind: StructuralPlaceKind::OperationResult {
                producer: instruction(1),
                structural_type,
            },
        }],
        result: abstract_operations::AbstractFunctionResult::Unit,
        declared_places: BTreeSet::from([place(1), place(9)]),
        entry_claim_declarations: Vec::new(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: BTreeSet::new(),
        published_service_ceiling: Vec::new(),
        declared_service_reach: Vec::new(),
        facts: Vec::new(),
        blocks: vec![optimization_unit::OptimizationBlock {
            id: position(0).0,
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
            nodes: operations
                .into_iter()
                .map(|operation| optimization_unit::OptimizationNode {
                    operation,
                    provenance: Vec::new(),
                    fuel: Vec::new(),
                    effect: optimization_unit::EffectLink {
                        input: 0,
                        output: 0,
                    },
                    definitions: Vec::new(),
                    uses: Vec::new(),
                    successors: Vec::new(),
                    ownership: Vec::new(),
                })
                .collect(),
        }],
    }
}

#[test]
fn scalar_with_a_colliding_producer_identity_cannot_retire_a_descriptor() {
    let function = receiver_fixture(scalar(1), true);
    let regions = BorrowedRegions::new(&function, &BTreeMap::new());
    let loans = DescriptorLoans::new(&function, &regions);
    assert_eq!(loans.loans.len(), 1);
    assert!(loans.loans[0].live.contains(&position(2)));
    assert_eq!(
        loans.conflicted_operation(
            &regions,
            position(2).0,
            2,
            &function.blocks[0].nodes[2].operation,
            &[]
        ),
        Some(place(9))
    );
    let flow = DescriptorFlow::new(&function);
    assert_eq!(flow.steps[&position(0)].definitions, vec![place(1)]);
    assert!(flow.steps[&position(1)].definitions.is_empty());
}

#[test]
fn stored_dynamic_selection_is_an_escape_at_capture_not_at_later_dispatch() {
    let owner = MachineId::new(1).unwrap();
    let capture = O::StoreDynamicDescriptor {
        psi_operation: instruction(2),
        stored: abstract_operations::AbstractStoredDynamicDescriptor {
            selection: terminal_psi::TerminalDynamicConformanceSelection {
                owner,
                ordinal: 0,
                source: terminal_psi::StructuralArgument {
                    place: place(1),
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                },
                conformance_application_report_fingerprint: 0,
                conformance_application_commitment: Default::default(),
            },
            descriptor: terminal_psi::TerminalStoredDynamicDescriptor {
                owner,
                ordinal: 0,
                establishment_operation: instruction(2),
                selection_ordinal: 0,
                aggregate_type_identity: "test::Holder".into(),
                field_identity: "handler".into(),
            },
            application: terminal_psi::ClosedConformanceApplication {
                owner,
                declaration_identity: "test::Port".into(),
                telescope: Vec::new(),
                subject_identity: None,
                trait_identity: "test::Port".into(),
                trait_lifetime_arguments: Vec::new(),
                trait_arguments: Vec::new(),
                realization_callables: Vec::new(),
                rows: Vec::new(),
                report_fingerprint: 0,
                commitment: Default::default(),
            },
        },
    };
    let function = receiver_fixture(capture, false);
    let regions = BorrowedRegions::new(&function, &BTreeMap::new());
    let loans = DescriptorLoans::new(&function, &regions);
    assert_eq!(loans.loans.len(), 1);
    assert!(
        loans.loans[0].live.contains(&position(2)),
        "capture remains live without any subsequent descriptor read"
    );
    assert_eq!(
        loans.conflicted_operation(
            &regions,
            position(2).0,
            2,
            &function.blocks[0].nodes[2].operation,
            &[]
        ),
        Some(place(9))
    );
}
