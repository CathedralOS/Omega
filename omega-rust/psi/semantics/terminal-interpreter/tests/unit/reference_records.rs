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

fn returned_record_reference_module() -> TerminalModule {
    let mut module = record_reference_module();
    let caller = &mut module.machines[0];
    let mut make_view = caller.clone();
    make_view.id = machine_id(201);
    make_view.entry = block_id(201);
    make_view.contract.id = contract_id(201);
    make_view.structural_parameters[0].place = place_id(201);
    make_view.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(201),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(204),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(204),
                structural_type: structural_type_id(94),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(206),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(206),
                structural_type: structural_type_id(95),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(207),
            kind: semantic_vocabulary::StructuralPlaceKind::Result,
        },
    ];
    make_view.result =
        TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: place_id(207),
            structural_type: structural_type_id(95),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: vec![],
            projected_qualifications: vec![],
            reference_sources: vec![terminal_psi::StructuralReferenceResultSource {
                path: vec!["body".into()],
                source: StructuralArgument {
                    place: place_id(201),
                    path: vec![],
                    access: StructuralAccess::MutableBorrow,
                },
            }],
        });
    make_view.blocks.truncate(1);
    let block = &mut make_view.blocks[0];
    block.id = block_id(201);
    block.operations.truncate(2);
    for (operation, identity) in block.operations.iter_mut().zip([204, 206]) {
        operation.id = operation_id(identity);
        let OperationResult::Structural(result) = &mut operation.result else {
            panic!("owned construction");
        };
        result.place = place_id(identity);
        match &mut operation.kind {
            OperationKind::EstablishReference { source } => source.place = place_id(201),
            OperationKind::EstablishRecord { fields } => {
                let RecordFieldValue::Structural(source) = &mut fields[0].value else {
                    panic!("reference field");
                };
                source.place = place_id(204);
            }
            _ => panic!("reference then record construction"),
        }
    }
    block.terminator = Terminator::ReturnStructural {
        edge: edge_id(201),
        source: place_id(206),
        returned_claims: vec![],
        trivial_affine_discards: vec![],
    };
    caller
        .structural_places
        .retain(|place| place.id != place_id(94));
    caller.blocks[0].operations.remove(0);
    caller.blocks[0].operations[0].kind = OperationKind::CallStructural {
        callee: machine_id(201),
        structural_arguments: vec![StructuralArgument {
            place: place_id(91),
            path: vec![],
            access: StructuralAccess::MutableBorrow,
        }],
        claim_transfers: vec![],
        returned_claim_transfers: vec![],
        requirement_obligations: vec![],
        crash_continuations: vec![],
        selected_evidence: vec![],
    };
    module.machines.push(make_view);
    module
}

#[test]
fn returned_record_reference_preserves_caller_storage_and_once_only_calls() {
    execute_record_reference(
        returned_record_reference_module(),
        &[96, 204, 206, 97, 92, 93, 98],
    );
}

#[test]
fn returned_reference_leaves_follow_reversed_actual_arguments() {
    let mut module = returned_record_reference_module();
    let StructuralTypeShape::Record { fields } =
        &mut module.structural_types.last_mut().unwrap().shape
    else {
        panic!("View record");
    };
    let mut other = fields[0].clone();
    other.id = structural_field_id(305);
    other.identity = "other".into();
    fields.push(other);
    let caller = &mut module.machines[0];
    let mut parameter = caller.structural_parameters[0].clone();
    parameter.place = place_id(301);
    parameter.position = 1;
    caller.structural_parameters.push(parameter);
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(301),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let OperationKind::CallStructural {
        structural_arguments,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        panic!("make_view call");
    };
    structural_arguments.insert(
        0,
        StructuralArgument {
            place: place_id(301),
            path: vec![],
            access: StructuralAccess::MutableBorrow,
        },
    );
    let make_view = module.machines.last_mut().unwrap();
    let mut parameter = make_view.structural_parameters[0].clone();
    parameter.place = place_id(302);
    parameter.position = 1;
    make_view.structural_parameters.push(parameter);
    make_view.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(302),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(304),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(304),
                structural_type: structural_type_id(94),
            },
        },
    ]);
    let mut reference = make_view.blocks[0].operations[0].clone();
    reference.id = operation_id(304);
    let OperationResult::Structural(result) = &mut reference.result else {
        panic!("reference result");
    };
    result.place = place_id(304);
    let OperationKind::EstablishReference { source } = &mut reference.kind else {
        panic!("reference formation");
    };
    source.place = place_id(302);
    let OperationKind::EstablishRecord { fields } = &mut make_view.blocks[0].operations[1].kind
    else {
        panic!("record construction");
    };
    fields.push(RecordFieldInitializer {
        field: structural_field_id(305),
        value: RecordFieldValue::Structural(StructuralArgument {
            place: place_id(304),
            path: vec![],
            access: StructuralAccess::Owned,
        }),
    });
    make_view.blocks[0].operations.insert(1, reference);
    let TerminalMachineResult::Structural(result) = &mut make_view.result else {
        panic!("record result");
    };
    result
        .reference_sources
        .push(terminal_psi::StructuralReferenceResultSource {
            path: vec!["other".into()],
            source: StructuralArgument {
                place: place_id(302),
                path: vec![],
                access: StructuralAccess::MutableBorrow,
            },
        });
    let mut crossed = module.clone();
    let TerminalMachineResult::Structural(result) =
        &mut crossed.machines.last_mut().unwrap().result
    else {
        panic!("record result");
    };
    result.reference_sources[0].source.place = place_id(302);
    result.reference_sources[1].source.place = place_id(201);
    assert!(
        encode_module(&crossed).is_err(),
        "equal reference types cannot substitute source correspondence"
    );
    execute_record_reference_values(
        module,
        &[96, 204, 304, 206, 97, 92, 93, 98],
        &[(1, 1), (2, 7)],
        1,
    );
}

#[test]
fn returned_child_record_restores_its_parent_only_after_disposal() {
    let mut module = returned_record_reference_module();
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(216),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: operation_id(216),
            structural_type: structural_type_id(95),
        },
    });
    let mut child_call = caller.blocks[0].operations[0].clone();
    child_call.id = operation_id(216);
    let OperationResult::Structural(result) = &mut child_call.result else {
        panic!("record result");
    };
    result.place = place_id(216);
    let OperationKind::CallStructural {
        structural_arguments,
        ..
    } = &mut child_call.kind
    else {
        panic!("record call");
    };
    structural_arguments[0].place = place_id(96);
    structural_arguments[0].path = vec!["body".into(), StructuralPathSegment::Referent];
    let mut child_write = caller.blocks[0].operations[1].clone();
    child_write.id = operation_id(217);
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut child_write.kind
    else {
        panic!("writer call");
    };
    structural_arguments[0].place = place_id(216);
    let operations = caller.blocks[0].operations.split_off(1);
    let terminator = caller.blocks[0].terminator.clone();
    caller.blocks[0]
        .operations
        .extend([child_call, child_write]);
    caller.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(216),
        target: block_id(216),
        arguments: vec![],
        structural_arguments: vec![],
        trivial_affine_discards: vec![place_id(216)],
        residual_affine_discards: vec![],
    };
    caller.blocks.push(Block {
        id: block_id(216),
        parameters: vec![],
        structural_parameters: vec![],
        operations,
        terminator,
    });
    let mut premature_parent_use = module.clone();
    let caller = &mut premature_parent_use.machines[0];
    let parent_write = caller.blocks[2].operations.remove(0);
    caller.blocks[0].operations.push(parent_write);
    assert!(
        encode_module(&premature_parent_use).is_err(),
        "a returned child must not restore its parent before disposal"
    );
    execute_record_reference(
        module,
        &[96, 204, 206, 216, 204, 206, 217, 92, 93, 97, 92, 93, 98],
    );
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
    execute_record_reference_values(module, executed_operations, &[(1, 7)], 7);
}

fn execute_record_reference_values(
    module: TerminalModule,
    executed_operations: &[u64],
    initial_and_final_values: &[(u128, u128)],
    returned: u128,
) {
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
            &initial_and_final_values
                .iter()
                .enumerate()
                .map(|(argument_index, _)| TerminalStructuralValue {
                    opaque_identity: 94 + argument_index as u64,
                    structural_type: structural_type_id(91),
                    qualifications: vec![],
                    path: vec![],
                })
                .collect::<Vec<_>>(),
            &initial_and_final_values
                .iter()
                .enumerate()
                .map(
                    |(argument_index, &(initial, _))| TerminalStructuralPrimitiveValue {
                        argument_index: u32::try_from(argument_index).unwrap(),
                        value: scalar(initial),
                    },
                )
                .collect::<Vec<_>>(),
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
                assert_eq!(result, TerminalExecutionResult::Scalar(scalar(returned)));
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("unexpected crash: {crash:?}"),
        }
    }
    assert_eq!(
        execution.structural_primitive_values(),
        initial_and_final_values
            .iter()
            .enumerate()
            .map(
                |(argument_index, &(_, expected))| TerminalStructuralPrimitiveValue {
                    argument_index: u32::try_from(argument_index).unwrap(),
                    value: scalar(expected),
                }
            )
            .collect::<Vec<_>>()
    );
    for &operation in executed_operations {
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation_id(operation)))
                .expect("every authored operation executes")
                .executions(),
            executed_operations
                .iter()
                .filter(|&&candidate| candidate == operation)
                .count() as u64,
            "resumption must not repeat operation {operation}",
        );
    }
}
