//! A record moves reference permission, then releases it before restored use.

use super::*;
use terminal_psi::{RecordFieldInitializer, RecordFieldValue};

fn record_reference_module() -> TerminalModule {
    let mut module = reference_release_module();
    module
        .machines
        .push(write_only_primitive_call_module().machines.remove(1));
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(95),
        identity: "test::View".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(95),
                identity: "body".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(structural_type_id(94)),
            }],
        },
    });
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(96),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(96),
            structural_type: structural_type_id(95),
        },
    });
    caller.blocks[0].operations.truncate(1);
    caller.blocks[0].operations.extend([
        Operation {
            static_reach_binding: None,
            id: operation_id(96),
            result: OperationResult::Structural(StructuralOperationResult {
                place: place_id(96),
                structural_type: structural_type_id(95),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                claims: vec![],
            }),
            kind: OperationKind::EstablishRecord {
                fields: vec![RecordFieldInitializer {
                    field: structural_field_id(95),
                    value: RecordFieldValue::Structural(StructuralArgument {
                        place: place_id(94),
                        path: vec![],
                        access: StructuralAccess::Owned,
                    }),
                }],
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(97),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: machine_id(92),
                arguments: vec![],
                structural_arguments: vec![StructuralArgument {
                    place: place_id(96),
                    path: vec!["body".into(), StructuralPathSegment::Referent],
                    access: StructuralAccess::WriteOnlyBorrow,
                }],
                claim_transfers: vec![],
                requirement_obligations: vec![],
                crash_continuations: vec![],
            },
        },
    ]);
    caller.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(96),
        target: block_id(96),
        arguments: vec![],
        structural_arguments: vec![],
        trivial_affine_discards: vec![place_id(96)],
        residual_affine_discards: vec![],
    };
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(99),
        scalar_type,
        qualifications: Default::default(),
    });
    caller.blocks.push(Block {
        id: block_id(96),
        structural_parameters: vec![],
        parameters: vec![],
        operations: vec![Operation {
            static_reach_binding: None,
            id: operation_id(98),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(98),
                scalar_type,
                qualifications: Default::default(),
            }),
            kind: OperationKind::PrimitiveScalarRead {
                source: place_id(91),
            },
        }],
        terminator: Terminator::Return {
            edge: edge_id(97),
            value: value_id(98),
            cleanup_actions: vec![],
        },
    });
    module
}

#[test]
fn record_reference_preserves_original_storage_across_calls_and_cleanup() {
    execute_record_reference(record_reference_module(), &[94, 96, 97, 92, 93, 98]);
}

#[test]
fn nested_record_moves_the_same_reference_permission_twice() {
    let mut module = record_reference_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(101),
        identity: "test::Envelope".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(101),
                identity: "inner".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(structural_type_id(95)),
            }],
        },
    });
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(101),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(101),
            structural_type: structural_type_id(101),
        },
    });
    let mut outer = caller.blocks[0].operations[1].clone();
    outer.id = operation_id(101);
    let OperationResult::Structural(result) = &mut outer.result else {
        panic!("record result");
    };
    result.place = place_id(101);
    result.structural_type = structural_type_id(101);
    outer.kind = OperationKind::EstablishRecord {
        fields: vec![RecordFieldInitializer {
            field: structural_field_id(101),
            value: RecordFieldValue::Structural(StructuralArgument {
                place: place_id(96),
                path: vec![],
                access: StructuralAccess::Owned,
            }),
        }],
    };
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[2].kind
    else {
        panic!("writer call");
    };
    structural_arguments[0].place = place_id(101);
    structural_arguments[0].path.insert(0, "inner".into());
    caller.blocks[0].operations.insert(2, outer);
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut caller.blocks[0].terminator
    else {
        panic!("cleanup edge");
    };
    *trivial_affine_discards = vec![place_id(101)];
    execute_record_reference(module, &[94, 96, 101, 97, 92, 93, 98]);
}

#[test]
fn record_reference_supports_a_child_reborrow_after_the_owned_move() {
    let mut module = record_reference_module();
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(99),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(99),
            structural_type: structural_type_id(94),
        },
    });
    let mut child = caller.blocks[0].operations[0].clone();
    child.id = operation_id(99);
    let OperationResult::Structural(result) = &mut child.result else {
        panic!("reference result");
    };
    result.place = place_id(99);
    child.kind = OperationKind::EstablishReference {
        source: StructuralArgument {
            place: place_id(96),
            path: vec!["body".into(), StructuralPathSegment::Referent],
            access: StructuralAccess::MutableBorrow,
        },
    };
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[2].kind
    else {
        panic!("writer call");
    };
    structural_arguments[0].place = place_id(99);
    structural_arguments[0].path = vec![StructuralPathSegment::Referent];
    caller.blocks[0].operations.insert(2, child);
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(100),
        result: OperationResult::Unit,
        kind: OperationKind::ReleaseReference {
            source: place_id(99),
        },
    });
    execute_record_reference(module, &[94, 96, 99, 97, 92, 93, 100, 98]);
}

fn execute_record_reference(module: TerminalModule, executed_operations: &[u64]) {
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = |value| TerminalScalarValue::Integer {
        scalar_type: integer,
        value: IntegerValue::Unsigned(value),
    };
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[TerminalStructuralValue {
                opaque_identity: 94,
                structural_type: structural_type_id(91),
                qualifications: vec![],
                path: vec![],
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: scalar(1),
            }],
        )
        .expect("record-owned reference custody verifies from source-free bytes");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    loop {
        let status = execution.resume(&mut meter).unwrap();
        match status {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                assert_eq!(execution.resume(&mut meter).unwrap(), status);
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Scalar(scalar(7)));
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("unexpected crash: {crash:?}"),
        }
    }
    assert_eq!(
        execution.structural_primitive_values(),
        vec![TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: scalar(7)
        }]
    );
    for &operation in executed_operations {
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation_id(operation)))
                .expect("every authored operation executes")
                .executions(),
            1,
            "resumption must not repeat operation {operation}",
        );
    }
}
