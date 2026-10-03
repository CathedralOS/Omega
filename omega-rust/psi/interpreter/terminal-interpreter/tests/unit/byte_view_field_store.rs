//! `StructuralByteViewFieldStore` replaces a shared view field's descriptor
//! with an established literal's: later field observations see the literal's
//! exact octets and extent, and a second store replaces the first.
use super::{
    AdmissionProfile, BindingRelevance, ByteSequenceCarrier, IntegerSign, IntegerType,
    IntegerValue, Operation, OperationKind, OperationResult, ProofBundle, ScalarType,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldId, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalFuelMeter, TerminalMachineResult, TerminalModule, TerminalScalarValue,
    TerminalStructuralValue, Terminator, ValueDeclaration, byte_sequence_literal_module, edge_id,
    encode_module, encode_proof_section, operation_id, place_id, structural_type_id, value_id,
    verify_module,
};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};

fn length_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn operation(identity: u64, kind: OperationKind, result: OperationResult) -> Operation {
    Operation {
        id: operation_id(identity),
        kind,
        result,
        static_reach_binding: None,
        suspension_crossing: None,
    }
}

/// `place(5)`: an exclusive `Room { label: &[u8] in Utf8 }` parameter.
/// `place(1)` holds `"Gate"` and `place(3)` holds `"Doorway"`; both replace
/// `label` in turn, and the machine returns the field's final length.
fn module() -> TerminalModule {
    let mut module = byte_sequence_literal_module(b"Gate".to_vec());
    module.boundary_machines.clear();
    let mut utf8 = terminal_psi::ByteSequencePredicateSet::EMPTY;
    utf8.insert(terminal_psi::ByteSequencePredicate::ValidUtf8);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "test::Room".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "label".into(),
                relevance: BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::PredicateByteSequence {
                    carrier: ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    },
                    predicates: utf8,
                },
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(5),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(2),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(3),
            kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 1,
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(5),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
    ]);
    let store = |identity, source| {
        operation(
            identity,
            OperationKind::StructuralByteViewFieldStore {
                destination: place_id(5),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
                source: place_id(source),
                predicate_obligation: None,
            },
            OperationResult::Unit,
        )
    };
    let block = &mut machine.blocks[0];
    block.operations.truncate(1);
    block.operations.extend([
        store(2, 1),
        operation(
            3,
            OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(3),
                bytes: b"Doorway".to_vec(),
                qualifications: Vec::new(),
            },
            OperationResult::Unit,
        ),
        store(4, 3),
        operation(
            5,
            OperationKind::StructuralByteSequenceFieldLength {
                source: place_id(5),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
            },
            OperationResult::Scalar(ValueDeclaration {
                id: value_id(5),
                scalar_type: ScalarType::Integer(length_type()),
                qualifications: Default::default(),
            }),
        ),
    ]);
    block.terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(5),
        cleanup_actions: Vec::new(),
    };
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(6),
        scalar_type: ScalarType::Integer(length_type()),
        qualifications: Default::default(),
    });
    module
}

#[test]
fn a_literal_replaces_the_field_descriptor_and_its_extent() {
    let module = module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_section(&module, &ProofBundle::default()).unwrap();
    let input = [TerminalStructuralValue {
        opaque_identity: 999,
        structural_type: structural_type_id(2),
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &input,
            ..Default::default()
        },
    )
    .unwrap();
    // One unit per operation: some stop falls between the two stores and
    // observes the first literal in place; resumption completes the
    // replacement. No stop observes a partial descriptor.
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut observed = Vec::new();
    loop {
        match execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap()
        {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                observed.push(
                    execution
                        .structural_byte_sequence_field(
                            999,
                            &[],
                            StructuralFieldId::new(1).unwrap(),
                        )
                        .map(<[u8]>::to_vec),
                );
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(
                    result,
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                        scalar_type: length_type(),
                        value: IntegerValue::Unsigned(7),
                    })
                );
                break;
            }
            other => panic!("unexpected status {other:?}"),
        }
    }
    assert!(observed.contains(&Some(b"Gate".to_vec())), "{observed:?}");
    assert!(
        observed
            .iter()
            .flatten()
            .all(|bytes| bytes == b"Gate" || bytes == b"Doorway"),
        "{observed:?}"
    );
    assert_eq!(
        execution.structural_byte_sequence_field(999, &[], StructuralFieldId::new(1).unwrap()),
        Some(&b"Doorway"[..])
    );
}
