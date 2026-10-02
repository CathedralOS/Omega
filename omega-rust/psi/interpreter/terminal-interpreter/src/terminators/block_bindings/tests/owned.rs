use crate::effects::AcceptTerminalEffects;
use crate::terminators::block_bindings::tests::{
    BlockId, EdgeId, PlaceId, ScalarType, StructuralAccess, StructuralMultiplicity,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, SuccessorEdge, TerminalExecution,
    TerminalExecutionStatus, TerminalFuelMeter, TerminalInterpretError, TerminalMachineResult,
    TerminalScalarValue, Terminator, ValueDeclaration, ValueId, execution, successor,
};
use crate::terminators::block_bindings::{
    StructuralAffineDiscard, bind_affine_frontier, remove_affine_root,
};
use crate::values::StructuralRuntimePlace;
use crate::values::StructuralScalarRuntimeField;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, OperationId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, Operation, OperationKind, OperationResult, StructuralFieldDeclaration,
    StructuralFieldType, StructuralPlaceDeclaration, TerminalAffineCleanupAction,
};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn owned_successor() -> SuccessorEdge {
    let mut edge = successor();
    for argument in &mut edge.structural_arguments {
        argument.access = StructuralAccess::Owned;
    }
    edge
}

fn jump(edge: SuccessorEdge) -> Terminator {
    Terminator::Jump {
        edge: edge.edge,
        target: edge.target,
        arguments: edge.arguments,
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: edge.structural_arguments,
        trivial_affine_discards: edge.trivial_affine_discards,
        residual_affine_discards: Vec::new(),
    }
}

fn owned_execution(
    terminator: Terminator,
    multiplicity: StructuralMultiplicity,
) -> TerminalExecution {
    let mut execution = execution(terminator);
    let structural_type = StructuralTypeId::new(1).unwrap();
    execution.mutate_structural_type(structural_type, |declaration| {
        declaration.shape = StructuralTypeShape::Record {
            fields: [
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                ScalarType::Boolean,
            ]
            .into_iter()
            .enumerate()
            .map(|(position, scalar_type)| StructuralFieldDeclaration {
                id: StructuralFieldId::new(position as u64 + 1).unwrap(),
                identity: format!("field{position}"),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(scalar_type),
            })
            .collect(),
        };
    });
    execution.byte_sequence_values.clear();
    let target = std::sync::Arc::get_mut(&mut execution.program)
        .map(|program| &mut program.machines)
        .unwrap()
        .get_mut(&execution.current_machine)
        .unwrap()
        .blocks
        .get_mut(&BlockId::new(2).unwrap())
        .unwrap();
    let mut sources = target.structural_parameters.clone();
    for source in &mut sources {
        source.access = StructuralAccess::Owned;
        source.multiplicity = multiplicity;
    }
    target.structural_parameters = sources.clone();
    for (position, parameter) in target.structural_parameters.iter_mut().enumerate() {
        parameter.place = PlaceId::new(position as u64 + 3).unwrap();
    }
    let integer_read = ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(3).unwrap(),
        scalar_type: unsigned(0).scalar_type(),
    };
    target.operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(integer_read),
            kind: OperationKind::IntegerStructuralField {
                path: Vec::new(),
                source: PlaceId::new(3).unwrap(),
                field: StructuralFieldId::new(1).unwrap(),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(4).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                path: Vec::new(),
                source: PlaceId::new(4).unwrap(),
                field: StructuralFieldId::new(2).unwrap(),
            },
        },
    ];
    target.terminator = Terminator::Return {
        edge: EdgeId::new(2).unwrap(),
        value: integer_read.id,
        cleanup_actions: if multiplicity == StructuralMultiplicity::Affine {
            [4, 3]
                .map(|place| TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(place).unwrap()))
                .to_vec()
        } else {
            Vec::new()
        },
    };
    let target_id = target.id;
    let parameters = target.parameters.clone();
    let target_parameters = target.structural_parameters.clone();
    let machine = std::sync::Arc::get_mut(&mut execution.program)
        .map(|program| &mut program.machines)
        .unwrap()
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.parameters = parameters;
    machine.structural_parameters = sources;
    machine.structural_places = machine
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: false,
            },
        })
        .chain(
            target_parameters
                .iter()
                .map(|parameter| StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: target_id,
                        position: parameter.position,
                    },
                }),
        )
        .collect();
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(9).unwrap(),
        scalar_type: integer_read.scalar_type,
    });
    execution.live_affine_frontier =
        bind_affine_frontier(&machine.structural_parameters, &execution.structural_values).unwrap();
    for (place, integer, boolean) in [(1, 7, true), (2, 42, false)] {
        let root = &execution.structural_values[&PlaceId::new(place).unwrap()];
        for (field, value) in [
            (1, unsigned(integer)),
            (2, TerminalScalarValue::Boolean(boolean)),
        ] {
            execution.structural_scalar_fields.insert(
                StructuralScalarRuntimeField::member(
                    StructuralRuntimePlace::from(root),
                    StructuralFieldId::new(field).unwrap(),
                ),
                value,
            );
        }
    }
    execution
}

#[test]
fn case_payload_copy_checks_the_tag_and_transfers_independent_storage() {
    use crate::structural_inputs::case_membership::StructuralCaseContents;
    use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, StructuralCaseId};
    use terminal_psi::{
        StructuralCaseDeclaration, StructuralOperationResult, StructuralTypeDeclaration,
    };

    let mut edge = owned_successor();
    let source = PlaceId::new(1).unwrap();
    let copied = PlaceId::new(5).unwrap();
    edge.structural_arguments[0].place = copied;
    edge.structural_arguments[1].place = PlaceId::new(2).unwrap();
    let mut execution = owned_execution(jump(edge.clone()), StructuralMultiplicity::Unrestricted);
    let record_type = StructuralTypeId::new(1).unwrap();
    let sum_type = StructuralTypeId::new(2).unwrap();
    let active = StructuralCaseId::new(1).unwrap();
    let payload = StructuralFieldId::new(3).unwrap();
    execution.insert_structural_type(StructuralTypeDeclaration {
        id: sum_type,
        identity: "Choice".into(),
        shape: StructuralTypeShape::Sum {
            cases: [1, 2]
                .into_iter()
                .map(|identity| StructuralCaseDeclaration {
                    id: StructuralCaseId::new(identity).unwrap(),
                    identity: format!("Case{identity}"),
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(identity + 2).unwrap(),
                        identity: "payload".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(record_type),
                    }],
                })
                .collect(),
        },
    });
    let root = execution.structural_values.get_mut(&source).unwrap();
    root.structural_type = sum_type;
    let root = StructuralRuntimePlace::from(&*root);
    execution.structural_cases.insert(
        root.clone(),
        StructuralCaseContents {
            structural_type: sum_type,
            case: active,
        },
    );
    let mut member = root.clone();
    member
        .path
        .push(terminal_psi::StructuralPathSegment::Field("payload".into()));
    for field in [1, 2] {
        let field = StructuralFieldId::new(field).unwrap();
        let value = execution
            .structural_scalar_fields
            .remove(&StructuralScalarRuntimeField::member(root.clone(), field))
            .unwrap();
        execution.structural_scalar_fields.insert(
            StructuralScalarRuntimeField::member(member.clone(), field),
            value,
        );
    }
    let operation = Operation {
        id: OperationId::new(30).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        result: OperationResult::Structural(StructuralOperationResult {
            place: copied,
            structural_type: record_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::StructuralCaseLeafCopy {
            source,
            path: vec![Segment::Case(active), Segment::Field(payload)],
        },
    };
    // Both cases spell the member identically. Its name alone cannot authorize
    // reading the inactive payload, and a failed check must publish no copy.
    for path in [
        vec![
            Segment::Case(StructuralCaseId::new(2).unwrap()),
            Segment::Field(StructuralFieldId::new(4).unwrap()),
        ],
        vec![Segment::Case(active)],
        vec![
            Segment::Case(active),
            Segment::Field(StructuralFieldId::new(99).unwrap()),
        ],
        vec![Segment::Field(payload)],
    ] {
        let mut forged = operation.clone();
        forged.kind = OperationKind::StructuralCaseLeafCopy { source, path };
        let cursor = execution.local_structural_identities.cursor();
        assert!(matches!(
            execution.execute_structural_leaf_copy(&forged),
            Err(TerminalInterpretError::VerifiedOperationMalformed)
        ));
        assert!(!execution.structural_values.contains_key(&copied));
        assert_eq!(execution.local_structural_identities.cursor(), cursor);
    }
    let mut affine = operation.clone();
    let OperationResult::Structural(result) = &mut affine.result else {
        panic!("structural copy");
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    assert!(matches!(
        execution.execute_structural_leaf_copy(&affine),
        Err(TerminalInterpretError::AffineProjectionNotRepresentable)
    ));
    execution.execute_structural_leaf_copy(&operation).unwrap();
    let copy_root = StructuralRuntimePlace::from(&execution.structural_values[&copied]);
    assert_ne!(copy_root.opaque_identity, root.opaque_identity);
    let integer_field = StructuralFieldId::new(1).unwrap();
    execution.structural_scalar_fields.insert(
        StructuralScalarRuntimeField::member(copy_root, integer_field),
        unsigned(99),
    );
    assert_eq!(
        execution.structural_scalar_fields
            [&StructuralScalarRuntimeField::member(member, integer_field)],
        unsigned(7)
    );
    let machine = std::sync::Arc::get_mut(&mut execution.program)
        .map(|program| &mut program.machines)
        .unwrap()
        .get_mut(&execution.current_machine)
        .unwrap();
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: copied,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation.id,
            structural_type: record_type,
        },
    });
    machine
        .blocks
        .get_mut(&machine.entry)
        .unwrap()
        .operations
        .push(operation);
    execution
        .prepare_block_bindings(
            edge.target,
            &edge.arguments,
            &edge.structural_arguments,
            true,
        )
        .unwrap()
        .commit(&mut execution);
    let arrival =
        StructuralRuntimePlace::from(&execution.structural_values[&PlaceId::new(3).unwrap()]);
    assert_eq!(
        execution.structural_scalar_fields
            [&StructuralScalarRuntimeField::member(arrival, integer_field)],
        unsigned(99)
    );
}

#[test]
fn unrestricted_copy_staging_failure_leaves_backing_and_identity_cursor_unchanged() {
    for exhausted in [false, true] {
        let mut execution = owned_execution(
            jump(owned_successor()),
            StructuralMultiplicity::Unrestricted,
        );
        if exhausted {
            // One fresh identity can be staged, but the second must fail.
            execution
                .local_structural_identities
                .commit_cursor(Some(u64::MAX));
        }
        let values = execution.structural_values.clone();
        let fields = execution.structural_scalar_fields.clone();
        let frontier = execution.live_affine_frontier.clone();
        let cursor = execution.local_structural_identities.cursor();
        let edge = owned_successor();
        let prepared = execution.prepare_block_bindings(
            edge.target,
            &edge.arguments,
            &edge.structural_arguments,
            true,
        );
        if exhausted {
            assert!(matches!(
                prepared,
                Err(TerminalInterpretError::StructuralIdentityExhausted)
            ));
        } else {
            let prepared = prepared.unwrap();
            assert!(
                prepared
                    .validate_discards(&execution, &[PlaceId::new(999).unwrap()], &[])
                    .is_err()
            );
        }
        assert_eq!(execution.structural_values, values);
        assert_eq!(execution.structural_scalar_fields, fields);
        assert_eq!(execution.live_affine_frontier, frontier);
        assert_eq!(execution.local_structural_identities.cursor(), cursor);
    }
}

#[test]
fn an_explicit_discard_precedes_rebinding_the_same_destination_place() {
    let mut edge = owned_successor();
    let destination = PlaceId::new(3).unwrap();
    edge.trivial_affine_discards.push(destination);
    let mut execution = owned_execution(jump(edge), StructuralMultiplicity::Affine);
    let incoming = execution.structural_values[&PlaceId::new(2).unwrap()].clone();
    let mut previous = incoming.clone();
    previous.opaque_identity = 1000;
    execution.structural_values.insert(destination, previous);
    execution
        .live_affine_frontier
        .insert(StructuralAffineDiscard {
            place: destination,
            path: Vec::new(),
            structural_type: incoming.structural_type,
        });
    let before = execution.structural_values.clone();
    let frontier = execution.live_affine_frontier.clone();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values, before);
    assert_eq!(execution.live_affine_frontier, frontier);
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_values[&destination], incoming);
    assert_eq!(execution.structural_values.len(), 2);
    assert_eq!(execution.live_affine_frontier.len(), 2);
}

#[test]
fn invalid_discard_suffix_cannot_partially_commit_a_valid_prefix() {
    for invalid in [3, 99] {
        let mut edge = owned_successor();
        let destination = PlaceId::new(3).unwrap();
        edge.trivial_affine_discards = vec![destination, PlaceId::new(invalid).unwrap()];
        let mut execution = owned_execution(jump(edge), StructuralMultiplicity::Affine);
        let mut value = execution.structural_values[&PlaceId::new(2).unwrap()].clone();
        value.opaque_identity = 1000;
        execution
            .live_affine_frontier
            .insert(StructuralAffineDiscard {
                place: destination,
                path: Vec::new(),
                structural_type: value.structural_type,
            });
        execution.structural_values.insert(destination, value);
        let before = execution.structural_values.clone();
        let frontier = execution.live_affine_frontier.clone();
        assert!(
            execution
                .resume(
                    &mut TerminalFuelMeter::with_allowance(1),
                    &mut AcceptTerminalEffects
                )
                .is_err()
        );
        assert_eq!(execution.structural_values, before);
        assert_eq!(execution.live_affine_frontier, frontier);
    }
}

#[test]
fn malformed_owned_handoffs_leave_all_custody_uncommitted() {
    for mutation in [
        "duplicate",
        "alias",
        "missing",
        "partial",
        "source mode",
        "target owner",
        "qualification",
        "projected",
        "claim",
        "transfer and discard",
    ] {
        let mut edge = owned_successor();
        let mut execution = owned_execution(jump(edge.clone()), StructuralMultiplicity::Affine);
        let source = edge.structural_arguments[0].place;
        match mutation {
            "duplicate" => edge.structural_arguments[1].place = source,
            "alias" => {
                let identity = execution.structural_values[&source].opaque_identity;
                execution
                    .structural_values
                    .get_mut(&edge.structural_arguments[1].place)
                    .unwrap()
                    .opaque_identity = identity;
            }
            "missing" => {
                remove_affine_root(&mut execution.live_affine_frontier, source);
            }
            "partial" => {
                remove_affine_root(&mut execution.live_affine_frontier, source);
                execution
                    .live_affine_frontier
                    .insert(StructuralAffineDiscard {
                        place: source,
                        path: vec!["part".into()],
                        structural_type: StructuralTypeId::new(1).unwrap(),
                    });
            }
            "source mode" => {
                std::sync::Arc::get_mut(&mut execution.program)
                    .map(|program| &mut program.machines)
                    .unwrap()
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_parameters[1]
                    .access = StructuralAccess::SharedBorrow
            }
            "target owner" => {
                std::sync::Arc::get_mut(&mut execution.program)
                    .map(|program| &mut program.machines)
                    .unwrap()
                    .get_mut(&execution.current_machine)
                    .unwrap()
                    .structural_places[2]
                    .kind = StructuralPlaceKind::BlockParameter {
                    block: BlockId::new(1).unwrap(),
                    position: 0,
                }
            }
            "qualification" => execution
                .structural_values
                .get_mut(&source)
                .unwrap()
                .qualifications
                .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap()),
            "projected" => edge.structural_arguments[0].path.push("part".into()),
            "claim" => {
                execution.live_claims.insert(
                    semantic_vocabulary::ClaimId::new(1).unwrap(),
                    crate::execution::LiveClaim {
                        place: Some(source),
                        path: Vec::new(),
                        multiplicity: Some(StructuralMultiplicity::Linear),
                    },
                );
            }
            "transfer and discard" => edge.trivial_affine_discards.push(source),
            _ => unreachable!(),
        }
        let original = execution.structural_values.clone();
        let frontier = execution.live_affine_frontier.clone();
        let claims = execution.live_claims.clone();
        let fields = execution.structural_scalar_fields.clone();
        let result = execution
            .prepare_block_bindings(
                edge.target,
                &edge.arguments,
                &edge.structural_arguments,
                true,
            )
            .and_then(|bindings| {
                bindings.validate_discards(&execution, &edge.trivial_affine_discards, &[])
            });
        assert!(result.is_err(), "accepted {mutation}");
        assert_eq!(execution.structural_values, original);
        assert_eq!(execution.live_affine_frontier, frontier);
        assert_eq!(execution.live_claims, claims);
        assert_eq!(execution.structural_scalar_fields, fields);
    }
}

#[test]
fn array_successors_capture_all_payloads_before_replacing_destinations() {
    use crate::scalar_array::TerminalScalarArrayValue;
    use terminal_psi::StructuralTypeDeclaration;

    let mut edge = owned_successor();
    edge.structural_arguments[0].place = PlaceId::new(4).unwrap();
    edge.structural_arguments[1].place = PlaceId::new(3).unwrap();
    let mut execution = owned_execution(jump(edge.clone()), StructuralMultiplicity::Unrestricted);
    let array_type = StructuralTypeId::new(1).unwrap();
    let element_type = StructuralTypeId::new(2).unwrap();
    execution.insert_structural_type(StructuralTypeDeclaration {
        id: element_type,
        identity: "u64".into(),
        shape: StructuralTypeShape::PrimitiveScalar(unsigned(0).scalar_type()),
    });
    execution.mutate_structural_type(array_type, |declaration| {
        declaration.shape = StructuralTypeShape::FixedArray {
            element: element_type,
            length: 2,
        };
    });
    execution.structural_values.clear();
    execution.structural_scalar_fields.clear();
    let first = PlaceId::new(3).unwrap();
    let second = PlaceId::new(4).unwrap();
    let first_value = TerminalScalarArrayValue {
        structural_type: array_type,
        elements: vec![unsigned(30), unsigned(40)],
    };
    let second_value = TerminalScalarArrayValue {
        structural_type: array_type,
        elements: vec![unsigned(70), unsigned(80)],
    };
    execution
        .scalar_array_values
        .insert(first, first_value.clone());
    execution
        .scalar_array_values
        .insert(second, second_value.clone());
    let bindings = execution
        .prepare_block_bindings(
            edge.target,
            &edge.arguments,
            &edge.structural_arguments,
            true,
        )
        .unwrap();
    bindings.validate_discards(&execution, &[], &[]).unwrap();
    bindings.commit(&mut execution);
    assert_eq!(execution.scalar_array_values[&first], second_value);
    assert_eq!(execution.scalar_array_values[&second], first_value);

    // A malformed later payload cannot partially replace the first binding.
    execution
        .scalar_array_values
        .get_mut(&first)
        .unwrap()
        .elements
        .pop();
    let before = execution.scalar_array_values.clone();
    assert!(
        execution
            .prepare_block_bindings(
                edge.target,
                &edge.arguments,
                &edge.structural_arguments,
                true,
            )
            .is_err()
    );
    assert_eq!(execution.scalar_array_values, before);
}
