//! Custody-free values inside an unranked cycle. A boundary call's scalar
//! result is an ordinary value, as an in-module call's is, and an
//! unrestricted scalar case is a copy payload. Re-entering either producer on
//! every traversal moves no custody, so neither closes the cycle fence. An
//! affine scalar case keeps its confinement rule.

use super::{
    AdmissionProfile, ControlCycleIneligibility, IntegerSign, IntegerType, ModuleError, Operation,
    OperationId, OperationKind, OperationResult, PlaceId, ProofBundle, ScalarType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TerminalModule,
    ValueDeclaration, ValueId, id, unranked_scalar_cycle, validate_module,
    verify_module_for_interpretation,
};
use semantic_vocabulary::{BoundaryMachineId, StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, BoundaryMachineDeclaration, BoundaryMachineResult, ScalarCaseField,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralOperationResult,
};

fn owned_byte_cycle() -> TerminalModule {
    use terminal_psi::{ByteSequenceCarrier, control_flow::ByteSequenceOperand};

    let mut module = unranked_scalar_cycle();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1, StructuralTypeId::new),
            identity: "test::LiteralBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        },
        StructuralTypeDeclaration {
            id: id(2, StructuralTypeId::new),
            identity: "test::OwnedBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 8,
            }),
        },
    ];
    let machine = &mut module.machines[0];
    let length_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for (ordinal, bytes) in [(1, b"a".as_slice()), (2, b"b".as_slice())] {
        let place = id(ordinal, PlaceId::new);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: (ordinal - 1) as u32,
                structural_type: id(1, StructuralTypeId::new),
            },
        });
        machine.blocks[0].operations.extend([
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(ordinal * 2 - 1, OperationId::new),
                result: OperationResult::Unit,
                kind: OperationKind::EstablishByteSequenceLiteral {
                    destination: place,
                    bytes: bytes.to_vec(),
                    qualifications: Vec::new(),
                },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(ordinal * 2, OperationId::new),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(ordinal, ValueId::new),
                    scalar_type: length_type,
                }),
                kind: OperationKind::ByteSequenceLength { source: place },
            },
        ]);
    }
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(5, OperationId::new),
            structural_type: id(2, StructuralTypeId::new),
        },
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(5, OperationId::new),
        result: OperationResult::Structural(StructuralOperationResult {
            place: id(3, PlaceId::new),
            structural_type: id(2, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishByteSequenceConcatenation {
            left: ByteSequenceOperand::Whole {
                source: id(1, PlaceId::new),
                length: id(1, ValueId::new),
            },
            right: ByteSequenceOperand::Whole {
                source: id(2, PlaceId::new),
                length: id(2, ValueId::new),
            },
            capacity_obligation: id(1, semantic_vocabulary::ObligationId::new),
        },
    });
    module
}

#[test]
fn cyclic_owned_bytes_reconstruct_capacity_without_waiving_evidence() {
    use terminal_verifier::{
        reconstruct_interpretable_operation_obligations, validate_module_for_interpretation,
    };

    let module = owned_byte_cycle();
    validate_module(&module).expect("fresh unrestricted backing is legal on every traversal");
    let obligations = reconstruct_interpretable_operation_obligations(
        validate_module_for_interpretation(&module).unwrap(),
    )
    .unwrap();
    assert_eq!(obligations.len(), 1);
    assert_eq!(
        obligations[0].obligation.proposition,
        terminal_semantics::byte_sequence_concatenation_capacity_goal(
            &module.machines[0].blocks[0].operations[4],
            8,
        )
        .unwrap(),
    );
    assert!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err(),
        "cycle admission does not discharge the capacity obligation",
    );
}

#[test]
fn cyclic_owned_bytes_require_exact_dominating_lengths() {
    use terminal_psi::control_flow::ByteSequenceOperand;

    for corruption in ["other source length", "length after construction"] {
        let mut module = owned_byte_cycle();
        let operations = &mut module.machines[0].blocks[0].operations;
        if corruption == "other source length" {
            let OperationKind::EstablishByteSequenceConcatenation { left, .. } =
                &mut operations[4].kind
            else {
                unreachable!()
            };
            // Both literals have length one, but equal numbers do not convey
            // the other source's current-length identity.
            *left = ByteSequenceOperand::Whole {
                source: id(1, PlaceId::new),
                length: id(2, ValueId::new),
            };
        } else {
            operations.swap(3, 4);
        }
        assert!(validate_module(&module).is_err(), "{corruption}");
    }
}

#[test]
fn cyclic_owned_result_rebind_cannot_disturb_a_carried_shared_loan() {
    use semantic_vocabulary::{BlockId, EdgeId};
    use terminal_psi::{
        Block, StructuralAccess, StructuralArgument, StructuralParameterDeclaration, Terminator,
    };

    let mut module = owned_byte_cycle();
    // Shared successor loans already admit completed records, whereas direct
    // loans of concatenation results have a separate source-admission gap.
    // Exercise the generic publication fence through an admitted constructor.
    module.structural_types = vec![StructuralTypeDeclaration {
        id: id(2, StructuralTypeId::new),
        identity: "test::OwnedRecord".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }];
    let machine = &mut module.machines[0];
    machine
        .structural_places
        .retain(|place| place.id == id(3, PlaceId::new));
    machine.blocks[0].operations.drain(..4);
    machine.blocks[0].operations[0].kind = OperationKind::EstablishRecord { fields: Vec::new() };
    let loop_block = machine.entry;
    let preheader = id(2, BlockId::new);
    let borrowed = id(5, PlaceId::new);
    let initial = id(4, PlaceId::new);
    let mut initial_constructor = machine.blocks[0].operations[0].clone();
    initial_constructor.id = id(6, OperationId::new);
    let OperationResult::Structural(result) = &mut initial_constructor.result else {
        unreachable!()
    };
    result.place = initial;
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: initial,
            kind: StructuralPlaceKind::OperationResult {
                producer: id(6, OperationId::new),
                structural_type: id(2, StructuralTypeId::new),
            },
        },
        StructuralPlaceDeclaration {
            id: borrowed,
            kind: StructuralPlaceKind::BlockParameter {
                block: loop_block,
                position: 0,
            },
        },
    ]);
    machine.blocks[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: borrowed,
            position: 0,
            is_self: false,
            structural_type: id(2, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let argument = |place| StructuralArgument {
        place,
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
    };
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[0].terminator else {
        unreachable!()
    };
    when_true.structural_arguments.push(argument(borrowed));
    machine.entry = preheader;
    machine.blocks.push(Block {
        id: preheader,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![initial_constructor],
        terminator: Terminator::Jump {
            edge: id(13, EdgeId::new),
            target: loop_block,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![argument(initial)],
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    });
    validate_module(&module)
        .expect("the unrelated initial owner stays pinned, not the fresh result");
    let Terminator::Conditional { when_true, .. } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    when_true.structural_arguments[0] = argument(id(3, PlaceId::new));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::SharedStructuralLoanDisturbed {
            operation: id(5, OperationId::new),
            place: id(3, PlaceId::new),
        }),
    );
}

fn scalar_boundary_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let boundary = id(1, BoundaryMachineId::new);
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary,
        identity: "Console::write_status".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Scalar(scalar_type),
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(30, ValueId::new),
            scalar_type,
        }),
        kind: OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });
    module
}

#[test]
fn cyclic_boundary_scalar_result_is_custody_free() {
    let module = scalar_boundary_cycle();
    validate_module(&module).expect("a scalar boundary result does not close the cycle fence");
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the cyclic scalar boundary call verifies");
}

fn scalar_case_cycle(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let sum_type = id(1, StructuralTypeId::new);
    let set_case = id(1, StructuralCaseId::new);
    let value_field = id(1, StructuralFieldId::new);
    let place = id(1, PlaceId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: sum_type,
        identity: "test::Flag".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: set_case,
                    identity: "Set".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: value_field,
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
                StructuralCaseDeclaration {
                    id: id(2, StructuralCaseId::new),
                    identity: "Clear".into(),
                    fields: Vec::new(),
                },
            ],
        },
    });
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id(31, OperationId::new),
            structural_type: sum_type,
        },
    });
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(31, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: sum_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case: set_case,
                fields: vec![ScalarCaseField {
                    field: value_field,
                    value: id(10, ValueId::new),
                    range_obligation: None,
                }],
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(32, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(32, ValueId::new),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::StructuralCaseMembership {
                source: place,
                path: Vec::new(),
                case: set_case,
            },
        },
    ];
    module
}

#[test]
fn cyclic_unrestricted_scalar_case_is_a_copy_payload() {
    let module = scalar_case_cycle(StructuralMultiplicity::Unrestricted);
    validate_module(&module).expect("an unrestricted scalar case does not close the cycle fence");
    verify_module_for_interpretation(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the cyclic unrestricted scalar case verifies");
}

#[test]
fn cyclic_affine_scalar_case_stays_confined() {
    let module = scalar_case_cycle(StructuralMultiplicity::Affine);
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::ControlCycleIneligible {
                reason: ControlCycleIneligibility::BlockOperation { .. },
                ..
            })
        ),
        "an unconfined affine scalar case keeps the fence: {:?}",
        validate_module(&module)
    );
}
