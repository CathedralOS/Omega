//! Owned byte mutation retains declared predicates, not initializer accidents.
use super::*;
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{ByteSequencePredicate, StructuralOperationResult};

fn operation(ordinal: u64, result: OperationResult, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(ordinal),
        result,
        kind,
    }
}

fn scalar(ordinal: u64, bits: u16) -> OperationResult {
    OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(ordinal),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap()),
    })
}

fn owned_write(bytes: &[u8], predicate: Option<ByteSequencePredicate>) -> TerminalModule {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Literal".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::Owned".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            }),
        },
    ];
    if let Some(predicate) = predicate {
        module.structural_domains.push(StructuralDomainDeclaration {
            id: id(1),
            semantic_domain: id(1),
            identity: "test::BytePredicate".into(),
            carrier: id(2),
            byte_predicate: Some(predicate),
            content_projection: None,
            establishment_routes: Vec::new(),
        });
    }
    let machine = &mut module.machines[0];
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: id(2),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 1,
                structural_type: id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: id(3),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(5),
                structural_type: id(2),
            },
        },
    ];
    machine.blocks[0].operations = vec![
        operation(
            1,
            OperationResult::Unit,
            OperationKind::EstablishByteSequenceLiteral {
                destination: id(1),
                bytes: bytes.to_vec(),
                qualifications: Vec::new(),
            },
        ),
        operation(
            2,
            scalar(1, 64),
            OperationKind::ByteSequenceLength { source: id(1) },
        ),
        operation(
            3,
            OperationResult::Unit,
            OperationKind::EstablishByteSequenceLiteral {
                destination: id(2),
                bytes: Vec::new(),
                qualifications: Vec::new(),
            },
        ),
        operation(
            4,
            scalar(2, 64),
            OperationKind::ByteSequenceLength { source: id(2) },
        ),
        operation(
            5,
            OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: id(3),
                structural_type: id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: predicate.map(|_| vec![id(1)]).unwrap_or_default(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishByteSequenceConcatenation {
                left: ByteSequenceOperand::Whole {
                    source: id(1),
                    length: id(1),
                },
                right: ByteSequenceOperand::Whole {
                    source: id(2),
                    length: id(2),
                },
                capacity_obligation: id(1),
            },
        ),
        operation(
            6,
            scalar(3, 64),
            OperationKind::ByteSequenceLength { source: id(3) },
        ),
        operation(
            7,
            scalar(4, 64),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        operation(
            8,
            scalar(5, 8),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        ),
        operation(
            9,
            OperationResult::Unit,
            OperationKind::ByteSequenceWrite {
                destination: id(3),
                index: id(4),
                value: id(5),
                length: id(3),
                obligation: id(2),
            },
        ),
    ];
    module
}

fn owned_byte_boundary_call() -> TerminalModule {
    let mut module = owned_write(b"abc", Some(ByteSequencePredicate::ValidUtf8));
    module.structural_domains.push(StructuralDomainDeclaration {
        id: id(2),
        semantic_domain: id(1),
        identity: "test::ViewBytePredicate".into(),
        carrier: id(1),
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            crash_routes: Vec::new(),
            id: id(1),
            identity: "test::observe".into(),
            attachment: None,
            parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: id(20),
                position: 0,
                is_self: false,
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: vec![id(2)],
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    module.machines[0].blocks[0].operations.push(operation(
        10,
        OperationResult::Unit,
        OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: id(1),
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: id(3),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            completion_receipts: Vec::new(),
        },
    ));
    module
}

fn owned_byte_return(predicate: Option<ByteSequencePredicate>) -> TerminalModule {
    let mut module = owned_write(b"Gate", predicate);
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.truncate(5);
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(4),
        kind: StructuralPlaceKind::Result,
    });
    machine.result = terminal_psi::TerminalMachineResult::Structural(
        terminal_psi::StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: id(4),
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: predicate.map(|_| vec![id(1)]).unwrap_or_default(),
            projected_qualifications: Vec::new(),
        },
    );
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: id(1),
        source: id(3),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

#[test]
fn owned_byte_return_retains_exact_backing_and_capacity_obligation() {
    for predicate in [None, Some(ByteSequencePredicate::ValidUtf8)] {
        let module = owned_byte_return(predicate);
        validate_module(&module).expect("owned concatenation has exact claim-free result custody");
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        assert_eq!(obligations.len(), 1);
        assert_eq!(
            obligations[0].obligation.proposition,
            terminal_semantics::byte_sequence_concatenation_capacity_goal(
                &module.machines[0].blocks[0].operations[4],
                8,
            )
            .unwrap()
        );
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "return admission cannot replace the capacity certificate"
        );

        let mut smaller = module.clone();
        smaller.structural_types[1].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 3 });
        let smaller_obligations = reconstruct_operation_obligations(&smaller).unwrap();
        assert_ne!(
            obligations[0].obligation.proposition,
            smaller_obligations[0].obligation.proposition
        );
    }
}

#[test]
fn owned_byte_return_rejects_forged_source_and_qualification_custody() {
    for corruption in 0..6 {
        let mut module = owned_byte_return(Some(ByteSequencePredicate::ValidUtf8));
        let machine = &mut module.machines[0];
        match corruption {
            0 => {
                let Terminator::ReturnStructural { source, .. } = &mut machine.blocks[0].terminator
                else {
                    unreachable!()
                };
                *source = id(1);
            }
            1 => {
                machine.structural_places[2].kind = StructuralPlaceKind::OperationResult {
                    producer: id(4),
                    structural_type: id(2),
                }
            }
            2 => {
                let OperationResult::Structural(result) =
                    &mut machine.blocks[0].operations[4].result
                else {
                    unreachable!()
                };
                result.qualifications.clear();
            }
            3 => {
                let terminal_psi::TerminalMachineResult::Structural(result) = &mut machine.result
                else {
                    unreachable!()
                };
                result.qualifications.clear();
            }
            4 => {
                let OperationResult::Structural(result) =
                    &mut machine.blocks[0].operations[4].result
                else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }
            5 => {
                machine.blocks[0].operations.remove(4);
            }
            _ => unreachable!(),
        }
        validate_module(&module).unwrap_err();
    }
}

#[test]
fn owned_byte_return_forwards_an_exact_ordinary_call_result() {
    let mut module = owned_byte_return(Some(ByteSequencePredicate::ValidUtf8));
    let mut caller = module.machines[0].clone();
    caller.id = id(901);
    caller.contract.id = id(901);
    caller.entry = id(901);
    caller.blocks[0].id = id(901);
    caller
        .structural_places
        .retain(|place| place.id == id(3) || place.id == id(4));
    caller.structural_places[0].id = id(13);
    caller.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: id(15),
        structural_type: id(2),
    };
    caller.structural_places[1].id = id(14);
    let terminal_psi::TerminalMachineResult::Structural(signature) = &mut caller.result else {
        unreachable!()
    };
    signature.place = id(14);
    caller.blocks[0].terminator = Terminator::ReturnStructural {
        edge: id(2),
        source: id(13),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let mut call = caller.blocks[0].operations[4].clone();
    call.id = id(15);
    let OperationResult::Structural(result) = &mut call.result else {
        unreachable!()
    };
    result.place = id(13);
    call.kind = OperationKind::CallStructural {
        callee: module.machines[0].id,
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    caller.blocks[0].operations = vec![call];
    caller.blocks[0].operations.push(operation(
        16,
        scalar(3, 64),
        OperationKind::ByteSequenceLength { source: id(13) },
    ));
    caller.blocks[0].operations.push(operation(
        17,
        scalar(4, 64),
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        },
    ));
    caller.blocks[0].operations.push(operation(
        18,
        scalar(5, 8),
        OperationKind::ByteSequenceRead {
            source: id(13),
            index: id(4),
            length: id(3),
            obligation: id(99),
        },
    ));
    module.machines.push(caller);
    validate_module(&module).expect("an ordinary call forwards its exact owned byte result");
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    let read = obligations
        .iter()
        .find(|row| row.obligation.id == id(99))
        .unwrap();
    let length = ScalarTerm::value(id(3), ScalarType::Integer(integer()));
    assert!(
        read.semantic_axioms
            .contains(&Proposition::LessOrEqual(length.clone(), count(8),))
    );
    assert!(
        !read
            .semantic_axioms
            .contains(&Proposition::Equal(length, count(8)))
    );
    let predicates =
        terminal_semantics::byte_sequence_source_predicates(&module, &module.machines[1], id(13))
            .expect("the exact callee predicate roster follows the completed bytes");
    assert!(
        predicates
            .predicates()
            .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
    );
    let mut premature = module.clone();
    let mut length = premature.machines[1].blocks[0].operations.remove(1);
    length.id = id(14);
    premature.machines[1].blocks[0].operations.insert(0, length);
    assert!(
        matches!(
            validate_module(&premature),
            Err(terminal_verifier::ModuleError::ByteSequenceViewNotEstablished { .. })
        ),
        "an exact source declaration does not make the call result available early"
    );
    let OperationResult::Structural(result) =
        &mut module.machines[1].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result.qualifications.clear();
    validate_module(&module).expect_err("forwarding does not manufacture a qualification");
}

fn owned_byte_call_argument() -> TerminalModule {
    let mut module = owned_byte_return(Some(ByteSequencePredicate::ValidUtf8));
    let mut caller = unit_module().machines.remove(0);
    caller.id = id(901);
    caller.contract.id = id(901);
    caller.entry = id(901);
    caller.blocks[0].id = id(901);
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(2),
        trivial_affine_discards: Vec::new(),
    };
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: id(13),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(15),
            structural_type: id(2),
        },
    });
    let mut produced = module.machines[0].blocks[0].operations[4].clone();
    produced.id = id(15);
    let OperationResult::Structural(result) = &mut produced.result else {
        unreachable!()
    };
    result.place = id(13);
    produced.kind = OperationKind::CallStructural {
        callee: module.machines[0].id,
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    caller.blocks[0].operations = vec![
        produced,
        operation(
            16,
            OperationResult::Unit,
            OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: id(902),
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: id(13),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ),
    ];
    let mut consumer = unit_module().machines.remove(0);
    consumer.id = id(902);
    consumer.contract.id = id(902);
    consumer.entry = id(902);
    consumer.blocks[0].id = id(902);
    consumer.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(3),
        trivial_affine_discards: Vec::new(),
    };
    consumer.structural_places.push(StructuralPlaceDeclaration {
        id: id(20),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    consumer
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(20),
            position: 0,
            is_self: false,
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: vec![id(1)],
            projected_qualifications: Vec::new(),
        });
    module.entry = caller.id;
    module.machines.extend([caller, consumer]);
    module
}

#[test]
fn owned_byte_call_argument_retains_completed_qualified_value() {
    let module = owned_byte_call_argument();
    validate_module(&module).expect("an ordinary call receives the completed qualified byte owner");
    assert_eq!(reconstruct_operation_obligations(&module).unwrap().len(), 1);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect_err("argument admission does not discharge the producer's capacity obligation");
}

#[test]
fn owned_byte_call_argument_rejects_forged_custody() {
    for corruption in 0..9 {
        let mut module = owned_byte_call_argument();
        match corruption {
            0 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[1].blocks[0].operations[0].result
                else {
                    unreachable!()
                };
                result.qualifications.clear();
            }
            1 => {
                module.machines[1].structural_places[0].kind =
                    StructuralPlaceKind::OperationResult {
                        producer: id(16),
                        structural_type: id(2),
                    }
            }
            2 => module.machines[1].blocks[0].operations.swap(0, 1),
            3 => {
                let OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } = &mut module.machines[1].blocks[0].operations[1].kind
                else {
                    unreachable!()
                };
                structural_arguments[0].access = StructuralAccess::MutableBorrow;
            }
            4 => {
                module.structural_types.push(StructuralTypeDeclaration {
                    id: id(3),
                    identity: "test::SmallerOwner".into(),
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        capacity: 4,
                    }),
                });
                let formal = &mut module.machines[2].structural_parameters[0];
                formal.structural_type = id(3);
                formal.qualifications.clear();
            }
            5 => {
                module.structural_domains.push(StructuralDomainDeclaration {
                    id: id(2),
                    semantic_domain: id(2),
                    identity: "test::NoNul".into(),
                    carrier: id(2),
                    byte_predicate: Some(ByteSequencePredicate::NoNul),
                    content_projection: None,
                    establishment_routes: Vec::new(),
                });
                module.machines[2].structural_parameters[0].qualifications = vec![id(2)];
            }
            6 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[1].blocks[0].operations[0].result
                else {
                    unreachable!()
                };
                result.structural_type = id(1);
            }
            7 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[1].blocks[0].operations[0].result
                else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }
            8 => {
                let OperationKind::CallStructural { callee, .. } =
                    &mut module.machines[1].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                *callee = id(902);
            }
            _ => unreachable!(),
        }
        validate_module(&module).expect_err(&format!("forged byte call argument {corruption}"));
    }
}

#[test]
fn owned_byte_boundary_view_preserves_current_qualified_home() {
    let module = owned_byte_boundary_call();
    validate_module(&module).expect("a NUL overwrite preserves the declared UTF-8 shared view");
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err()
    );

    let mut module = owned_byte_boundary_call();
    module.structural_domains[1].semantic_domain = id(2);
    module.structural_domains[1].byte_predicate = Some(ByteSequencePredicate::NoNul);
    validate_module(&module).expect_err("initial no-NUL bytes do not survive an owned write");

    let mut module = owned_byte_boundary_call();
    module.boundary_machines[0].structural_parameters[0].qualifications = vec![id(1)];
    validate_module(&module).expect_err("the formal needs its own exact carrier qualification");
}

#[test]
fn owned_byte_boundary_view_rejects_exclusive_and_unestablished_presentations() {
    for access in [StructuralAccess::MutableBorrow, StructuralAccess::Owned] {
        let mut module = owned_byte_boundary_call();
        module.boundary_machines[0].structural_parameters[0].access = access;
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[9].kind
        else {
            unreachable!()
        };
        structural_arguments[0].access = access;
        validate_module(&module).expect_err("the owned-home adapter grants only a shared loan");
    }
    let mut module = owned_byte_boundary_call();
    module.machines[0].blocks[0].operations.swap(4, 9);
    validate_module(&module).expect_err("a declaration is not an established owned home");
    let mut module = owned_byte_boundary_call();
    module.boundary_machines[0].structural_parameters[0].structural_type = id(2);
    module.boundary_machines[0].structural_parameters[0].qualifications = vec![id(1)];
    validate_module(&module).expect_err("a boundary view does not authorize a new owned ABI");
}

#[test]
fn owned_utf8_write_preserves_declared_goal_without_incidental_no_nul() {
    let module = owned_write(b"abc", Some(ByteSequencePredicate::ValidUtf8));
    validate_module(&module).expect("a qualified fresh owner admits indexed mutation");
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    let goal = &obligations
        .iter()
        .find(|row| row.obligation.id == id(2))
        .unwrap()
        .obligation
        .proposition;
    let Proposition::Conjunction(legs) = goal else {
        panic!("bound and ASCII: {goal:?}");
    };
    assert_eq!(legs.len(), 2, "UTF-8 does not declare no-NUL");
    let predicates =
        terminal_semantics::byte_sequence_source_predicates(&module, &module.machines[0], id(3))
            .unwrap();
    assert_eq!(
        predicates.predicates().collect::<Vec<_>>(),
        vec![ByteSequencePredicate::ValidUtf8]
    );
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "structural acceptance does not invent the capacity or indexed-write certificates"
    );
}

#[test]
fn owned_unqualified_write_drops_all_incidental_source_predicates() {
    let mut module = owned_write(b"abc", None);
    module.machines[0].blocks[0].operations[7].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(255),
    };
    validate_module(&module).expect("raw bytes may lose initial UTF-8 and ASCII");
    assert!(
        terminal_semantics::byte_sequence_source_predicates(&module, &module.machines[0], id(3))
            .unwrap()
            .is_empty()
    );
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert!(matches!(
        obligations
            .iter()
            .find(|row| row.obligation.id == id(2))
            .unwrap()
            .obligation
            .proposition,
        Proposition::LessThan(_, _)
    ));
}

#[test]
fn owned_utf8_write_rejects_multibyte_backing_and_invalid_initial_membership() {
    for bytes in ["é".as_bytes(), &[255]] {
        let module = owned_write(bytes, Some(ByteSequencePredicate::ValidUtf8));
        validate_module(&module)
            .expect_err("ASCII overwrite cannot split UTF-8 or bless invalid input");
    }
}

#[test]
fn owned_write_rejects_literal_destination_and_substituted_length() {
    for replace_destination in [true, false] {
        let mut module = owned_write(b"abc", Some(ByteSequencePredicate::ValidUtf8));
        let OperationKind::ByteSequenceWrite {
            destination,
            length,
            ..
        } = &mut module.machines[0].blocks[0].operations[8].kind
        else {
            unreachable!();
        };
        if replace_destination {
            *destination = id(1);
        } else {
            *length = id(1);
        }
        validate_module(&module)
            .expect_err("equal length does not grant another place's authority");
    }
}

#[test]
fn owned_write_rejects_use_before_establishment_and_wrong_domain_carrier() {
    let mut module = owned_write(b"abc", Some(ByteSequencePredicate::ValidUtf8));
    module.machines[0].blocks[0].operations.swap(4, 8);
    validate_module(&module).expect_err("the owned root must be established before use");
    let mut module = owned_write(b"abc", Some(ByteSequencePredicate::ValidUtf8));
    module.structural_domains[0].carrier = id(1);
    validate_module(&module)
        .expect_err("a same-spelled predicate on another carrier grants no membership");
}

#[test]
fn owned_byte_domain_family_keeps_distinct_carrier_membership() {
    let mut module = owned_write(b"abc", Some(ByteSequencePredicate::ValidUtf8));
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(3),
        identity: "test::OwnedTen".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
            capacity: 10,
        }),
    });
    let mut second = module.structural_domains[0].clone();
    second.id = id(2);
    second.identity = "test::BytePredicate::OwnedTen".into();
    second.carrier = id(3);
    module.structural_domains.push(second);
    validate_module(&module).expect("one byte predicate family admits distinct exact carriers");

    let mut wrong_qualification = module.clone();
    let OperationResult::Structural(result) =
        &mut wrong_qualification.machines[0].blocks[0].operations[4].result
    else {
        panic!("fixture establishes an owned result");
    };
    result.qualifications = vec![id(2)];
    validate_module(&wrong_qualification)
        .expect_err("a sibling carrier's family member cannot qualify this result");

    let mut duplicate_pair = module.clone();
    duplicate_pair.structural_domains[1].carrier = id(2);
    assert!(matches!(
        validate_module(&duplicate_pair),
        Err(terminal_verifier::ModuleError::InvalidStructuralDomainIdentity(_))
    ));
    let mut duplicate_id = module.clone();
    duplicate_id.structural_domains[1].id = id(1);
    assert!(matches!(
        validate_module(&duplicate_id),
        Err(terminal_verifier::ModuleError::DuplicateStructuralDomain(_))
    ));
    let mut duplicate_identity = module.clone();
    duplicate_identity.structural_domains[1].identity =
        duplicate_identity.structural_domains[0].identity.clone();
    assert!(matches!(
        validate_module(&duplicate_identity),
        Err(terminal_verifier::ModuleError::InvalidStructuralDomainIdentity(_))
    ));
    for predicate in [None, Some(ByteSequencePredicate::AsciiOnly)] {
        let mut conflicting_family = module.clone();
        conflicting_family.structural_domains[1].byte_predicate = predicate;
        assert!(matches!(
            validate_module(&conflicting_family),
            Err(terminal_verifier::ModuleError::InvalidStructuralDomainIdentity(_))
        ));
    }
}

#[test]
fn non_byte_domain_family_cannot_span_nominal_carriers() {
    let mut module = unit_module();
    for ordinal in [1, 2] {
        module.structural_types.push(StructuralTypeDeclaration {
            id: id(ordinal),
            identity: format!("test::Nominal{ordinal}"),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        });
        module.structural_domains.push(StructuralDomainDeclaration {
            id: id(ordinal),
            semantic_domain: id(1),
            identity: format!("test::NominalDomain{ordinal}"),
            carrier: id(ordinal),
            byte_predicate: None,
            content_projection: None,
            establishment_routes: Vec::new(),
        });
    }
    assert!(matches!(
        validate_module(&module),
        Err(terminal_verifier::ModuleError::InvalidStructuralDomainIdentity(_))
    ));
}

fn owned_byte_reads(
    bytes: &[u8],
    predicate: Option<ByteSequencePredicate>,
    written: bool,
) -> TerminalModule {
    let mut module = owned_write(bytes, predicate);
    let operations = &mut module.machines[0].blocks[0].operations;
    if written {
        // Preserve each declared predicate while discarding incidental ones.
        operations[7].kind = OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1),
        };
    } else {
        operations.pop();
    }
    for ordinal in [0, 1] {
        operations.push(operation(
            10 + ordinal,
            scalar(6 + ordinal, 8),
            OperationKind::ByteSequenceRead {
                source: id(3),
                index: id(4),
                length: id(3),
                obligation: id(3 + ordinal),
            },
        ));
    }
    module
}

#[test]
fn owned_byte_read_classes_are_snapshot_facts_not_index_authority() {
    let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let value = ScalarTerm::value(id(6), ScalarType::Integer(byte_type));
    let no_nul = Proposition::LessOrEqual(
        ScalarTerm::integer(byte_type, IntegerValue::Unsigned(1)).unwrap(),
        value.clone(),
    );
    let ascii = Proposition::LessOrEqual(
        value,
        ScalarTerm::integer(byte_type, IntegerValue::Unsigned(127)).unwrap(),
    );
    for (predicate, written, expected_ascii, expected_no_nul) in [
        (None, false, true, true),
        (None, true, false, false),
        (Some(ByteSequencePredicate::ValidUtf8), true, false, false),
        (Some(ByteSequencePredicate::AsciiOnly), true, true, false),
        (Some(ByteSequencePredicate::NoNul), true, false, true),
    ] {
        let module = owned_byte_reads(b"abc", predicate, written);
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        let first = obligations
            .iter()
            .find(|row| row.obligation.id == id(3))
            .unwrap();
        let second = obligations
            .iter()
            .find(|row| row.obligation.id == id(4))
            .unwrap();
        assert!(matches!(
            first.obligation.proposition,
            Proposition::LessThan(_, _)
        ));
        assert!(!first.semantic_axioms.contains(&ascii));
        assert!(!first.semantic_axioms.contains(&no_nul));
        assert_eq!(second.semantic_axioms.contains(&ascii), expected_ascii);
        assert_eq!(second.semantic_axioms.contains(&no_nul), expected_no_nul);
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err()
        );
    }
    let module = owned_byte_reads(
        "é".as_bytes(),
        Some(ByteSequencePredicate::ValidUtf8),
        false,
    );
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert!(
        !obligations
            .iter()
            .any(|row| row.semantic_axioms.contains(&ascii))
    );

    let mut wrong_length = owned_byte_reads(b"abc", None, false);
    let OperationKind::ByteSequenceRead { length, .. } =
        &mut wrong_length.machines[0].blocks[0].operations[8].kind
    else {
        panic!("first observation");
    };
    *length = id(1);
    reconstruct_operation_obligations(&wrong_length)
        .expect_err("equal-valued length from the literal cannot authorize an owned read");
}
