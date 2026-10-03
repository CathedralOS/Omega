//! Canonical calls retain owned backing while presenting a shared byte view.
use super::*;
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{BoundaryMachineResult, ByteSequenceCarrier, ByteSequencePredicate};

fn operation(ordinal: u64, result: OperationResult, kind: OperationKind) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result,
        kind,
        static_reach_binding: None,
        suspension_crossing: None,
    }
}

fn scalar(ordinal: u64, bits: u16) -> OperationResult {
    OperationResult::Scalar(ValueDeclaration {
        id: value_id(ordinal),
        qualifications: Default::default(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap()),
    })
}

fn owned_byte_call(boundary: bool) -> TerminalModule {
    let mut module = unit_fixture();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "owner".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            }),
        },
    ];
    module.structural_domains = [2, 1]
        .into_iter()
        .enumerate()
        .map(|(position, carrier)| StructuralDomainDeclaration {
            id: structural_domain_id(position as u64 + 1),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: format!("Utf8@{carrier}"),
            carrier: structural_type_id(carrier),
            byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
            content_projection: None,
            establishment_routes: Vec::new(),
        })
        .collect();
    let formal = StructuralParameterDeclaration {
        place: place_id(20),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: vec![structural_domain_id(2)],
        projected_qualifications: Vec::new(),
    };
    let argument = StructuralArgument {
        place: place_id(2),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    let call = if boundary {
        module.boundary_machines.push(BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            id: boundary_machine_id(1),
            identity: "observe".into(),
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![formal],
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            crash_routes: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        OperationKind::BoundaryCall {
            boundary: boundary_machine_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![argument],
            requirement_obligations: Vec::new(),
            completion_receipts: Vec::new(),
        }
    } else {
        let mut callee = module.machines[0].clone();
        callee.id = machine_id(901);
        callee.entry = block_id(901);
        callee.blocks[0].id = callee.entry;
        callee.contract.id = contract_id(901);
        callee.blocks[0].terminator = Terminator::ReturnUnit {
            edge: edge_id(901),
            trivial_affine_discards: Vec::new(),
        };
        callee.structural_parameters = vec![formal];
        callee.structural_places = vec![StructuralPlaceDeclaration {
            id: place_id(20),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        module.machines.push(callee);
        OperationKind::CallUnit {
            callee: machine_id(901),
            arguments: Vec::new(),
            structural_arguments: vec![argument],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }
    };
    let machine = &mut module.machines[0];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(1),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(3),
                structural_type: structural_type_id(2),
            },
        },
    ];
    let operand = ByteSequenceOperand::Whole {
        source: place_id(1),
        length: value_id(1),
    };
    machine.blocks[0].operations = vec![
        operation(
            1,
            OperationResult::Unit,
            OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(1),
                bytes: b"ab".to_vec(),
                qualifications: Vec::new(),
            },
        ),
        operation(
            2,
            scalar(1, 64),
            OperationKind::ByteSequenceLength {
                source: place_id(1),
            },
        ),
        operation(
            3,
            OperationResult::Structural(StructuralOperationResult {
                place: place_id(2),
                structural_type: structural_type_id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![structural_domain_id(1)],
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishByteSequenceConcatenation {
                left: operand.clone(),
                right: operand,
                capacity_obligation: obligation_id(1),
            },
        ),
        operation(
            4,
            scalar(2, 64),
            OperationKind::ByteSequenceLength {
                source: place_id(2),
            },
        ),
        operation(
            5,
            scalar(3, 64),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        operation(
            6,
            scalar(4, 8),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        operation(
            7,
            OperationResult::Unit,
            OperationKind::ByteSequenceWrite {
                destination: place_id(2),
                index: value_id(3),
                value: value_id(4),
                length: value_id(2),
                obligation: obligation_id(2),
            },
        ),
        operation(8, OperationResult::Unit, call),
    ];
    module
}

#[test]
fn owned_byte_shared_view_roundtrips_with_original_home_and_qualifications() {
    for boundary in [false, true] {
        let module = owned_byte_call(boundary);
        terminal_verifier::validate_module(&module)
            .expect("shared view agrees with current owned predicates");
        let bytes = encode_module(&module).expect("the original owned carrier remains canonical");
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
    }
}

#[test]
fn owned_byte_shared_view_codec_rejects_changed_custody_or_predicates() {
    for boundary in [false, true] {
        for mutation in 0..5 {
            let mut module = owned_byte_call(boundary);
            match mutation {
                0 => {
                    let formal = if boundary {
                        &mut module.boundary_machines[0].structural_parameters[0]
                    } else {
                        &mut module.machines[1].structural_parameters[0]
                    };
                    formal.access = StructuralAccess::MutableBorrow;
                    let (OperationKind::BoundaryCall {
                        structural_arguments,
                        ..
                    }
                    | OperationKind::CallUnit {
                        structural_arguments,
                        ..
                    }) = &mut module.machines[0].blocks[0].operations[7].kind
                    else {
                        unreachable!()
                    };
                    structural_arguments[0].access = StructuralAccess::MutableBorrow;
                }
                1 => {
                    module.structural_domains[1].semantic_domain =
                        semantic_vocabulary::DomainSemanticId::new(2).unwrap();
                    module.structural_domains[1].byte_predicate =
                        Some(ByteSequencePredicate::NoNul);
                }
                2 => {
                    let formal = if boundary {
                        &mut module.boundary_machines[0].structural_parameters[0]
                    } else {
                        &mut module.machines[1].structural_parameters[0]
                    };
                    formal.qualifications = vec![structural_domain_id(1)];
                }
                3 => {
                    module.machines[0].structural_places[1].kind =
                        StructuralPlaceKind::OperationResult {
                            producer: operation_id(1),
                            structural_type: structural_type_id(2),
                        }
                }
                4 => module.machines[0].blocks[0].operations.swap(2, 7),
                _ => unreachable!(),
            }
            assert!(
                encode_module(&module).is_err(),
                "boundary={boundary}, mutation={mutation}"
            );
        }
    }
}
