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
    birth.derivations = vec![(place(1), place(1))];
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
    child.derivations = vec![(place(1), place(2))];
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
    let mut function = receiver_fixture(scalar(1), true);
    // Source access is now independently reconstructed through the descriptor
    // producer. Supply the exact root declaration; the later scalar operation
    // still deliberately collides with the descriptor's producer identity.
    function
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: place(9),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place(9),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    let regions = BorrowedRegions::new(
        &function,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &crate::current_ownership::CallAccesses::default(),
    );
    let loans = DescriptorLoans::new(&function, &regions, &BTreeMap::new());
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
    let flow = DescriptorFlow::new(&function, &BTreeMap::new(), &regions);
    assert_eq!(flow.steps[&position(0)].definitions, vec![place(1)]);
    assert!(flow.steps[&position(1)].definitions.is_empty());
}

#[test]
fn record_observation_keeps_every_contained_descriptor_backing_live() {
    let descriptor_type = StructuralTypeId::new(1).unwrap();
    let record_type = StructuralTypeId::new(2).unwrap();
    let fields = [1, 2].map(|number| semantic_vocabulary::StructuralFieldId::new(number).unwrap());
    let declarations = [
        StructuralTypeDeclaration {
            id: descriptor_type,
            identity: "test::View".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                },
            ),
        },
        StructuralTypeDeclaration {
            id: record_type,
            identity: "test::Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| StructuralFieldDeclaration {
                        id: *field,
                        identity: format!("view{index}"),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(descriptor_type),
                    })
                    .collect(),
            },
        },
    ];
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    for observe_record in [false, true] {
        // Exercise the actual operation adapter, including two independent
        // descriptor births. No direct use of either child follows capture.
        let mut function = receiver_fixture(scalar(2), false);
        let mut second = function.blocks[0].nodes[0].clone();
        let O::ByteSequenceSubslice {
            psi_operation,
            result,
            source,
            ..
        } = &mut second.operation
        else {
            unreachable!()
        };
        *psi_operation = instruction(5);
        result.place = place(2);
        *source = place(10);
        let mut record_result = result.clone();
        record_result.place = place(3);
        record_result.structural_type = record_type;
        let mut record = second.clone();
        record.operation = O::EstablishRecord {
            psi_operation: instruction(6),
            result: record_result,
            fields: fields
                .iter()
                .enumerate()
                .map(|(index, field)| terminal_psi::RecordFieldInitializer {
                    field: *field,
                    value: terminal_psi::RecordFieldValue::Structural(
                        terminal_psi::StructuralArgument {
                            place: place(index as u64 + 1),
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        },
                    ),
                })
                .collect(),
        };
        let first_write = function.blocks[0].nodes[2].clone();
        let mut second_write = first_write.clone();
        if let O::ByteSequenceWrite {
            psi_operation,
            destination,
            ..
        } = &mut second_write.operation
        {
            *psi_operation = instruction(7);
            *destination = place(10);
        }
        let mut nodes = vec![
            function.blocks[0].nodes[0].clone(),
            second,
            record,
            first_write,
            second_write,
        ];
        if observe_record {
            for (index, field) in fields.iter().enumerate() {
                let mut observation = nodes[0].clone();
                observation.operation = O::StructuralByteSequenceFieldLength {
                    psi_operation: instruction(index as u64 + 8),
                    result: abstract_operations::AbstractResult {
                        value: value(index as u64 + 8),
                        scalar_type: ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .unwrap(),
                        ),
                    },
                    source: place(3),
                    path: Vec::new(),
                    field: *field,
                };
                nodes.push(observation);
            }
        }
        function.blocks[0].nodes = nodes;
        for (result, producer, structural_type) in [
            (place(2), instruction(5), descriptor_type),
            (place(3), instruction(6), record_type),
        ] {
            function
                .structural_places
                .push(terminal_psi::StructuralPlaceDeclaration {
                    id: result,
                    kind: StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    },
                });
            function.declared_places.insert(result);
        }
        function.declared_places.insert(place(10));
        let regions = BorrowedRegions::new(
            &function,
            &BTreeMap::new(),
            &types,
            &crate::current_ownership::CallAccesses::default(),
        );
        let loans = DescriptorLoans::new(&function, &regions, &types);
        assert_eq!(loans.loans.len(), 2);
        for (node, backing) in [(3, place(9)), (4, place(10))] {
            assert_eq!(
                loans.conflicted_operation(
                    &regions,
                    position(node).0,
                    node,
                    &function.blocks[0].nodes[node as usize].operation,
                    &[],
                ),
                observe_record.then_some(backing),
                "contained descriptor backing {backing:?}, observed: {observe_record}"
            );
        }
    }
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
    let regions = BorrowedRegions::new(
        &function,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &crate::current_ownership::CallAccesses::default(),
    );
    let loans = DescriptorLoans::new(&function, &regions, &BTreeMap::new());
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

fn projected_output_call_fixture(
    result: abstract_operations::AbstractStructuralCallResult,
    output_access: StructuralAccess,
    output_shape: terminal_psi::StructuralTypeShape,
    observe_after_write: bool,
) -> (
    PsiOptimizationFunction,
    Vec<terminal_psi::StructuralTypeDeclaration>,
) {
    use terminal_psi::{
        BindingRelevance, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let identity = |number| StructuralTypeId::new(number).unwrap();
    let call = O::CallStructural {
        psi_operation: instruction(2),
        callee: MachineId::new(2).unwrap(),
        arguments: Vec::new(),
        structural_arguments: vec![
            StructuralArgument {
                place: place(1),
                access: StructuralAccess::SharedBorrow,
                path: Vec::new(),
            },
            StructuralArgument {
                place: place(10),
                access: output_access,
                path: vec![StructuralPathSegment::Field("output".into())],
            },
        ],
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        result,
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let mut function = receiver_fixture(call, observe_after_write);
    // A shared subslice's access is reconstructed through its actual source,
    // not inferred from its unrestricted result row. Retain that source's
    // declaration beside the unrelated writable output parameter below.
    function
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place(9),
            position: 1,
            is_self: false,
            structural_type: identity(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place(9),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    function
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place(10),
            position: 0,
            is_self: false,
            structural_type: identity(3),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place(10),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    function.declared_places.insert(place(10));
    function
        .structural_parameters
        .sort_by_key(|parameter| parameter.position);
    let field = |number, name: &str, field_type| StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(number).unwrap(),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let types = vec![
        StructuralTypeDeclaration {
            id: identity(1),
            identity: "test::View".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                },
            ),
        },
        StructuralTypeDeclaration {
            id: identity(3),
            identity: "test::Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "output", StructuralFieldType::Structural(identity(4))),
                    field(2, "opaque", StructuralFieldType::Structural(identity(6))),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: identity(4),
            identity: "test::Output".into(),
            shape: output_shape,
        },
        StructuralTypeDeclaration {
            id: identity(5),
            identity: "test::Boolean".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
        StructuralTypeDeclaration {
            id: identity(6),
            identity: "test::OpaqueService".into(),
            shape: StructuralTypeShape::FusedServiceBinding,
        },
    ];
    (function, types)
}

fn plain_output_shape() -> terminal_psi::StructuralTypeShape {
    terminal_psi::StructuralTypeShape::Record {
        fields: vec![terminal_psi::StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            identity: "value".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
        }],
    }
}

#[test]
fn writable_plain_projection_does_not_escape_a_shared_call_operand() {
    use abstract_operations::AbstractStructuralCallResult;
    for result in [
        AbstractStructuralCallResult::Unit,
        AbstractStructuralCallResult::Scalar(abstract_operations::AbstractResult {
            value: value(8),
            scalar_type: ScalarType::Boolean,
        }),
    ] {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let (function, declarations) =
                projected_output_call_fixture(result.clone(), access, plain_output_shape(), false);
            let types = declarations
                .iter()
                .map(|declaration| (declaration.id, declaration))
                .collect();
            let regions = BorrowedRegions::new(
                &function,
                &BTreeMap::new(),
                &types,
                &crate::current_ownership::CallAccesses::default(),
            );
            let loans = DescriptorLoans::new(&function, &regions, &types);
            assert_eq!(loans.loans.len(), 1);
            assert!(
                loans.loans[0].live.contains(&position(1)),
                "the call observes the view"
            );
            assert!(
                !loans.loans[0].live.contains(&position(2)),
                "{result:?} {access:?}: no carrier can retain the alias after the call"
            );
            assert_eq!(
                loans.conflicted_operation(
                    &regions,
                    position(1).0,
                    1,
                    &function.blocks[0].nodes[1].operation,
                    &[],
                ),
                None,
                "the writable output is disjoint from the view backing"
            );
            assert_eq!(
                loans.conflicted_operation(
                    &regions,
                    position(2).0,
                    2,
                    &function.blocks[0].nodes[2].operation,
                    &[place(9)],
                ),
                None,
                "after the final observation the backing may be written or transferred"
            );
        }
    }
}

#[test]
fn alias_bearing_or_unresolved_writable_output_keeps_the_call_loan_alive() {
    use abstract_operations::AbstractStructuralCallResult;
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};
    let output = StructuralTypeId::new(4).unwrap();
    let scalar = StructuralTypeId::new(5).unwrap();
    let mut nested_view = plain_output_shape();
    let StructuralTypeShape::Record { fields } = &mut nested_view else {
        unreachable!()
    };
    fields[0].field_type =
        terminal_psi::StructuralFieldType::Structural(StructuralTypeId::new(1).unwrap());
    for (description, shape) in [
        (
            "reference",
            StructuralTypeShape::Reference {
                referent: scalar,
                access: StructuralAccess::SharedBorrow,
            },
        ),
        (
            "element view",
            StructuralTypeShape::ElementView { element: scalar },
        ),
        (
            "byte view",
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
        ),
        ("nested view", nested_view),
        ("opaque", StructuralTypeShape::FusedServiceBinding),
        (
            "recursive",
            StructuralTypeShape::FixedArray {
                element: output,
                length: 0,
            },
        ),
        ("missing", plain_output_shape()),
    ] {
        let (function, mut declarations) = projected_output_call_fixture(
            AbstractStructuralCallResult::Unit,
            StructuralAccess::MutableBorrow,
            shape,
            false,
        );
        if description == "missing" {
            declarations.retain(|declaration| declaration.id != output);
        }
        let types = declarations
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        let regions = BorrowedRegions::new(
            &function,
            &BTreeMap::new(),
            &types,
            &crate::current_ownership::CallAccesses::default(),
        );
        let loans = DescriptorLoans::new(&function, &regions, &types);
        assert_eq!(loans.loans.len(), 1);
        assert!(
            loans.loans[0].live.contains(&position(2)),
            "{description}: an escaping alias cannot expire at the call"
        );
        assert_eq!(
            loans.conflicted_operation(
                &regions,
                position(2).0,
                2,
                &function.blocks[0].nodes[2].operation,
                &[],
            ),
            Some(place(9)),
            "{description}"
        );
    }
}

#[test]
fn a_non_escaping_call_does_not_retire_later_descriptor_observations() {
    let (function, declarations) = projected_output_call_fixture(
        abstract_operations::AbstractStructuralCallResult::Unit,
        StructuralAccess::MutableBorrow,
        plain_output_shape(),
        true,
    );
    let types = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let regions = BorrowedRegions::new(
        &function,
        &BTreeMap::new(),
        &types,
        &crate::current_ownership::CallAccesses::default(),
    );
    let loans = DescriptorLoans::new(&function, &regions, &types);
    assert!(loans.loans[0].live.contains(&position(3)));
    assert_eq!(
        loans.conflicted_operation(
            &regions,
            position(2).0,
            2,
            &function.blocks[0].nodes[2].operation,
            &[],
        ),
        Some(place(9)),
        "a write before the final observation still disturbs the loan"
    );
}

#[test]
fn carried_view_call_observation_respects_mutable_child_lifetime() {
    use terminal_psi::{
        BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralArgument,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    };
    let view_type = StructuralTypeId::new(1).unwrap();
    let scalar_type = StructuralTypeId::new(2).unwrap();
    let record_type = StructuralTypeId::new(3).unwrap();
    let declarations = [
        StructuralTypeDeclaration {
            id: view_type,
            identity: "test::Elements".into(),
            shape: StructuralTypeShape::ElementView {
                element: scalar_type,
            },
        },
        StructuralTypeDeclaration {
            id: scalar_type,
            identity: "test::Boolean".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
        StructuralTypeDeclaration {
            id: record_type,
            identity: "test::Holder".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "view".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(view_type),
                }],
            },
        },
    ];
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    for (call_after_birth, direct_child) in [(false, false), (true, false), (true, true)] {
        let call = O::CallStructural {
            psi_operation: instruction(7),
            callee: MachineId::new(2).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: if direct_child { place(1) } else { place(3) },
                access: StructuralAccess::SharedBorrow,
                path: Vec::new(),
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            result: abstract_operations::AbstractStructuralCallResult::Unit,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        let mut function = receiver_fixture(call, true);
        function
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: place(9),
                position: 0,
                is_self: false,
                structural_type: view_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        function.structural_places.push(StructuralPlaceDeclaration {
            id: place(9),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
        let O::ByteSequenceSubslice { result, .. } = &function.blocks[0].nodes[0].operation else {
            unreachable!()
        };
        let mut mutable_result = result.clone();
        mutable_result.multiplicity = StructuralMultiplicity::Affine;
        let mut shared_result = result.clone();
        shared_result.place = place(2);
        let mut record_result = result.clone();
        record_result.place = place(3);
        record_result.structural_type = record_type;
        let mut mutable = function.blocks[0].nodes[0].clone();
        mutable.operation = O::ElementViewSubslice {
            psi_operation: instruction(1),
            result: mutable_result,
            source: place(9),
            start: value(1),
            end: value(2),
            length: value(3),
            obligation: ObligationId::new(1).unwrap(),
        };
        let mut shared = mutable.clone();
        shared.operation = O::ElementViewSubslice {
            psi_operation: instruction(5),
            result: shared_result,
            source: place(9),
            start: value(1),
            end: value(2),
            length: value(3),
            obligation: ObligationId::new(2).unwrap(),
        };
        let mut record = mutable.clone();
        record.operation = O::EstablishRecord {
            psi_operation: instruction(6),
            result: record_result,
            fields: vec![RecordFieldInitializer {
                field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                value: RecordFieldValue::Structural(StructuralArgument {
                    place: place(2),
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }),
            }],
        };
        let call = function.blocks[0].nodes[1].clone();
        let mut observation = function.blocks[0].nodes[3].clone();
        observation.operation = O::ElementViewLength {
            psi_operation: instruction(4),
            source: place(1),
            result: abstract_operations::AbstractResult {
                value: value(4),
                scalar_type: ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
            },
        };
        function.blocks[0].nodes = if call_after_birth {
            vec![shared, record, mutable, call, observation]
        } else {
            vec![shared, record, call, mutable, observation]
        };
        for (place, producer, structural_type) in [
            (place(2), instruction(5), view_type),
            (place(3), instruction(6), record_type),
        ] {
            function.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            });
            function.declared_places.insert(place);
        }
        let regions = BorrowedRegions::new(
            &function,
            &BTreeMap::new(),
            &types,
            &crate::current_ownership::CallAccesses::default(),
        );
        let loans = DescriptorLoans::new(&function, &regions, &types);
        assert_eq!(loans.loans.len(), 2);
        assert_eq!(loans.loans[0].access, StructuralAccess::SharedBorrow);
        assert_eq!(loans.loans[1].access, StructuralAccess::MutableBorrow);
        let call_position = if call_after_birth { 3 } else { 2 };
        assert_eq!(
            loans.conflicted_operation(
                &regions,
                position(call_position).0,
                call_position,
                &function.blocks[0].nodes[call_position as usize].operation,
                &[],
            ),
            (call_after_birth && !direct_child).then_some(place(3)),
            "carried view call after birth: {call_after_birth}, direct child: {direct_child}"
        );
    }
}
