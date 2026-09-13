use super::*;
use terminal_psi::{RecordFieldInitializer, RecordFieldValue};

fn integer() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
fn scalar(value: u64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value.into()),
    }
}
fn result(place: u64, structural_type: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        place: place_id(place),
        structural_type: structural_type_id(structural_type),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        claims: vec![],
    })
}
fn scalar_field(field: u64, value: u64) -> RecordFieldInitializer {
    RecordFieldInitializer {
        field: structural_field_id(field),
        value: RecordFieldValue::Scalar {
            value: value_id(value),
            range_obligation: None,
        },
    }
}
fn child_field(field: u64, place: u64) -> RecordFieldInitializer {
    RecordFieldInitializer {
        field: structural_field_id(field),
        value: RecordFieldValue::Structural(StructuralArgument {
            place: place_id(place),
            path: vec![],
            access: StructuralAccess::Owned,
        }),
    }
}
fn record_module() -> TerminalModule {
    let mut module = unit_module();
    let declaration = |id, name: &str, field_type| StructuralFieldDeclaration {
        id: structural_field_id(id),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "Child".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    declaration(1, "count", StructuralFieldType::Scalar(integer())),
                    declaration(
                        2,
                        "enabled",
                        StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "Parent".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    declaration(
                        1,
                        "child",
                        StructuralFieldType::Structural(structural_type_id(1)),
                    ),
                    declaration(2, "other", StructuralFieldType::Scalar(integer())),
                ],
            },
        },
    ];
    let mut read = module.machines[0].clone();
    read.id = machine_id(2);
    read.entry = block_id(2);
    read.blocks[0].id = block_id(2);
    read.contract.id = contract_id(2);
    read.attachment = Some(structural_type_id(1));
    read.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(10),
        position: 0,
        is_self: true,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    }];
    read.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(10),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: true,
        },
    }];
    let output = ValueDeclaration {
        id: value_id(10),
        scalar_type: integer(),
        qualifications: Default::default(),
    };
    read.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(11),
        ..output
    });
    read.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(10),
        result: OperationResult::Scalar(output),
        kind: OperationKind::IntegerStructuralField {
            source: place_id(10),
            field: structural_field_id(1),
        },
    }];
    read.blocks[0].terminator = Terminator::Return {
        edge: edge_id(10),
        value: value_id(10),
        cleanup_actions: vec![],
    };
    let caller = &mut module.machines[0];
    caller.parameters = vec![
        ValueDeclaration {
            id: value_id(1),
            scalar_type: integer(),
            qualifications: Default::default(),
        },
        ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        },
    ];
    caller.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(1),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(1),
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(2),
                structural_type: structural_type_id(2),
            },
        },
    ];
    caller.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: result(1, 1),
            kind: OperationKind::EstablishRecord {
                fields: vec![scalar_field(1, 1), scalar_field(2, 2)],
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: result(2, 2),
            kind: OperationKind::EstablishRecord {
                fields: vec![child_field(1, 1), scalar_field(2, 1)],
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(3),
                scalar_type: integer(),
                qualifications: Default::default(),
            }),
            kind: OperationKind::CallStructuralScalar {
                callee: machine_id(2),
                arguments: vec![],
                structural_arguments: vec![StructuralArgument {
                    place: place_id(2),
                    path: vec![StructuralPathSegment::Field("child".into())],
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: vec![],
                requirement_obligations: vec![],
                crash_continuations: vec![],
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(3),
        cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id(2))],
    };
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(4),
        scalar_type: integer(),
        qualifications: Default::default(),
    });
    module.machines.push(read);
    module
}

#[test]
fn nested_record_fields_survive_borrowed_call_and_every_fuel_pause() {
    let module = record_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let mut prior = bytes.clone();
    prior[10..12].copy_from_slice(&99_u16.to_le_bytes());
    assert!(matches!(
        decode_module(&prior),
        Err(terminal_codec::CodecError::UnsupportedVocabularyMarker(99))
    ));
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    for value in [0, u64::MAX] {
        let start = || {
            TerminalExecution::start_artifact(
                &bytes,
                &proof,
                &AdmissionProfile::default(),
                &[scalar(value), TerminalScalarValue::Boolean(true)],
            )
            .unwrap()
        };
        let expected =
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(value)));
        let mut execution = start();
        let mut meter = TerminalFuelMeter::with_allowance(100);
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        let total = meter.usage().total_units();
        for split in 0..total {
            let mut execution = start();
            let mut meter = TerminalFuelMeter::with_allowance(split);
            assert!(matches!(
                execution.resume(&mut meter).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            meter.replenish(total - split).unwrap();
            assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        }
    }
}

#[test]
fn record_replay_rejects_inexact_fields_and_child_custody() {
    let module = record_module();
    for corruption in 0..10 {
        let mut forged = module.clone();
        let OperationKind::EstablishRecord { fields } =
            &mut forged.machines[0].blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        match corruption {
            0 => fields.swap(0, 1),
            1 => {
                fields.pop();
            }
            2 => fields[1].field = fields[0].field,
            3 => fields[1] = scalar_field(2, 2),
            4 => fields[0] = child_field(1, 2),
            5 => {
                if let RecordFieldValue::Structural(argument) = &mut fields[0].value {
                    argument.access = StructuralAccess::SharedBorrow;
                }
            }
            6 => {
                if let RecordFieldValue::Structural(argument) = &mut fields[0].value {
                    argument
                        .path
                        .push(StructuralPathSegment::Field("count".into()));
                }
            }
            7 => forged.machines[0].blocks[0].operations.swap(0, 1),
            8 => {
                fields[1] = child_field(2, 1);
                let StructuralTypeShape::Record {
                    fields: declarations,
                } = &mut forged.structural_types[1].shape
                else {
                    unreachable!()
                };
                declarations[1].field_type = StructuralFieldType::Structural(structural_type_id(1));
            }
            9 => {
                let OperationResult::Structural(result) =
                    &mut forged.machines[0].blocks[0].operations[1].result
                else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Unrestricted;
            }
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &forged,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "corruption {corruption} accepted"
        );
    }
}

#[test]
fn record_range_obligation_is_reconstructed_before_establishment() {
    let mut module = record_module();
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            carrier,
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(100),
        )
        .unwrap(),
    );
    let OperationKind::EstablishRecord { fields } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let RecordFieldValue::Scalar {
        range_obligation, ..
    } = &mut fields[0].value
    else {
        unreachable!()
    };
    *range_obligation = Some(obligation_id(1));
    let term = ScalarTerm::value(value_id(1), integer());
    let mut bounds = vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(carrier, IntegerValue::Unsigned(0)).unwrap(),
            term.clone(),
        ),
        Proposition::LessOrEqual(
            term,
            ScalarTerm::integer(carrier, IntegerValue::Unsigned(100)).unwrap(),
        ),
    ];
    bounds.sort();
    let conclusion = Proposition::Conjunction(bounds);
    module.machines[0].contract.requires = vec![conclusion.clone()];
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let obligation = reconstructed
        .obligations()
        .iter()
        .find(|entry| entry.obligation.id == obligation_id(1))
        .unwrap();
    assert_eq!(obligation.obligation.proposition, conclusion);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        semantic_vocabulary::BoundedIntegerType::new(
            carrier,
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(99),
        )
        .unwrap(),
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn repeated_child_call_results_keep_independent_completed_storage() {
    let mut module = record_module();
    let mut construct = unit_module().machines.remove(0);
    construct.id = machine_id(3);
    construct.contract.id = contract_id(3);
    construct.entry = block_id(3);
    construct.blocks[0].id = block_id(3);
    construct.parameters = vec![
        ValueDeclaration {
            id: value_id(20),
            scalar_type: integer(),
            qualifications: Default::default(),
        },
        ValueDeclaration {
            id: value_id(21),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        },
    ];
    construct.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: place_id(21),
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
    });
    construct.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(20),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(20),
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(21),
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    construct.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(20),
        result: result(20, 1),
        kind: OperationKind::EstablishRecord {
            fields: vec![scalar_field(1, 20), scalar_field(2, 21)],
        },
    }];
    construct.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(20),
        source: place_id(20),
        returned_claims: vec![],
        trivial_affine_discards: vec![],
    };
    module.machines.push(construct);
    let call = |value| OperationKind::CallStructuralWithScalarArguments {
        callee: machine_id(3),
        arguments: vec![value_id(value), value_id(2)],
        structural_arguments: vec![],
        claim_transfers: vec![],
        returned_claim_transfers: vec![],
        requirement_obligations: vec![],
        crash_continuations: vec![],
    };
    let caller = &mut module.machines[0];
    caller.parameters.push(ValueDeclaration {
        id: value_id(5),
        scalar_type: integer(),
        qualifications: Default::default(),
    });
    caller.blocks[0].operations[0].kind = call(1);
    caller.blocks[0].operations[1].id = operation_id(3);
    caller.blocks[0].operations[2].id = operation_id(4);
    if let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
        &mut caller.structural_places[1].kind
    {
        *producer = operation_id(3);
    }
    caller.blocks[0].operations.insert(
        1,
        Operation {
            static_reach_binding: None,
            id: operation_id(2),
            result: result(3, 1),
            kind: call(5),
        },
    );
    caller.structural_places.insert(
        1,
        StructuralPlaceDeclaration {
            id: place_id(3),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(2),
                structural_type: structural_type_id(1),
            },
        },
    );
    caller.structural_places.sort_by_key(|place| place.id);
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place_id(3)));
    let baseline = module;
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Unrestricted,
    ] {
        let mut module = baseline.clone();
        if multiplicity == StructuralMultiplicity::Unrestricted {
            for machine in &mut module.machines {
                if let TerminalMachineResult::Structural(result) = &mut machine.result {
                    result.multiplicity = multiplicity;
                }
                for block in &mut machine.blocks {
                    for operation in &mut block.operations {
                        if let OperationResult::Structural(result) = &mut operation.result {
                            result.multiplicity = multiplicity;
                        }
                    }
                    if let Terminator::Return {
                        cleanup_actions, ..
                    } = &mut block.terminator
                    {
                        cleanup_actions.clear();
                    }
                }
            }
        }
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap();
        let bytes = encode_module(&module).unwrap();
        let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
        let start = || {
            TerminalExecution::start_artifact(
                &bytes,
                &proof,
                &AdmissionProfile::default(),
                &[
                    scalar(u64::MAX),
                    TerminalScalarValue::Boolean(true),
                    scalar(17),
                ],
            )
            .unwrap()
        };
        let expected =
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(u64::MAX)));
        let mut execution = start();
        let mut meter = TerminalFuelMeter::with_allowance(100);
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        let total = meter.usage().total_units();
        for split in 0..total {
            let mut execution = start();
            let mut meter = TerminalFuelMeter::with_allowance(split);
            assert!(matches!(
                execution.resume(&mut meter).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            meter.replenish(total - split).unwrap();
            assert_eq!(execution.resume(&mut meter).unwrap(), expected);
        }
    }
}

#[test]
fn nested_record_mutable_receiver_updates_exact_child_storage() {
    let mut module = record_module();
    module.machines[0].parameters.push(ValueDeclaration {
        id: value_id(5),
        scalar_type: integer(),
        qualifications: Default::default(),
    });
    let OperationKind::CallStructuralScalar {
        arguments,
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    arguments.push(value_id(5));
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    let callee = &mut module.machines[1];
    callee.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    callee.parameters.push(ValueDeclaration {
        id: value_id(12),
        scalar_type: integer(),
        qualifications: Default::default(),
    });
    callee.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            id: operation_id(11),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: place_id(10),
                path: vec![],
                field: structural_field_id(1),
                value: value_id(12),
            },
        },
    );
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let bytes = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[
            scalar(u64::MAX),
            TerminalScalarValue::Boolean(true),
            scalar(17),
        ],
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::with_allowance(100))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(17)))
    );
}

#[test]
fn unrestricted_record_children_remain_available_after_parent_construction() {
    let mut module = record_module();
    let caller = &mut module.machines[0];
    for operation in &mut caller.blocks[0].operations[..2] {
        let OperationResult::Structural(result) = &mut operation.result else {
            unreachable!()
        };
        result.multiplicity = StructuralMultiplicity::Unrestricted;
    }
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = place_id(1);
    structural_arguments[0].path.clear();
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut caller.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let bytes = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &bytes,
        &proof,
        &AdmissionProfile::default(),
        &[scalar(u64::MAX), TerminalScalarValue::Boolean(true)],
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::with_allowance(100))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(u64::MAX)))
    );
}

#[test]
fn owned_nested_record_call_copies_payload_before_mutating_its_child() {
    let mut module = record_module();
    for operation in &mut module.machines[0].blocks[0].operations[..2] {
        let OperationResult::Structural(result) = &mut operation.result else {
            unreachable!()
        };
        result.multiplicity = StructuralMultiplicity::Unrestricted;
    }
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    let mut writer = module.machines[1].clone();
    writer.id = machine_id(4);
    writer.entry = block_id(4);
    writer.blocks[0].id = block_id(4);
    writer.contract.id = contract_id(4);
    writer.structural_parameters[0].place = place_id(30);
    writer.structural_parameters[0].structural_type = structural_type_id(2);
    writer.attachment = Some(structural_type_id(2));
    writer.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    writer.structural_places[0].id = place_id(30);
    writer.result = TerminalMachineResult::Unit;
    writer.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(30),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(30),
                scalar_type: integer(),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(99),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(31),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: place_id(30),
                path: vec![StructuralPathSegment::Field("child".into())],
                field: structural_field_id(1),
                value: value_id(30),
            },
        },
    ];
    writer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(30),
        trivial_affine_discards: vec![],
    };
    let mut mutator = writer.clone();
    mutator.id = machine_id(3);
    mutator.entry = block_id(3);
    mutator.blocks[0].id = block_id(3);
    mutator.contract.id = contract_id(3);
    mutator.attachment = None;
    mutator.structural_parameters[0].place = place_id(20);
    mutator.structural_parameters[0].structural_type = structural_type_id(2);
    mutator.structural_parameters[0].is_self = false;
    mutator.structural_parameters[0].access = StructuralAccess::Owned;
    mutator.structural_places[0] = StructuralPlaceDeclaration {
        id: place_id(20),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    };
    let call = |operation, callee, place, path, access| Operation {
        static_reach_binding: None,
        id: operation_id(operation),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: machine_id(callee),
            arguments: vec![],
            structural_arguments: vec![StructuralArgument {
                place: place_id(place),
                path,
                access,
            }],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    };
    mutator.blocks[0].operations = vec![call(20, 4, 20, vec![], StructuralAccess::MutableBorrow)];
    mutator.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(20),
        trivial_affine_discards: vec![],
    };
    module.machines[0].blocks[0]
        .operations
        .insert(2, call(4, 3, 2, vec![], StructuralAccess::Owned));
    module.machines.extend([mutator, writer]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let bytes = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let start = || {
        TerminalExecution::start_artifact(
            &bytes,
            &proof,
            &AdmissionProfile::default(),
            &[scalar(u64::MAX), TerminalScalarValue::Boolean(true)],
        )
        .unwrap()
    };
    let expected =
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(scalar(u64::MAX)));
    let mut execution = start();
    let mut meter = TerminalFuelMeter::with_allowance(100);
    assert_eq!(execution.resume(&mut meter).unwrap(), expected);
    let total = meter.usage().total_units();
    for split in 0..total {
        let mut execution = start();
        let mut meter = TerminalFuelMeter::with_allowance(split);
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - split).unwrap();
        assert_eq!(execution.resume(&mut meter).unwrap(), expected);
    }
}
