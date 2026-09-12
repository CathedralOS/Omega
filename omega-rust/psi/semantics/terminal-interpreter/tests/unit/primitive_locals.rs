//! Primitive locals retain original backing across borrowed calls and reentry.

use super::*;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_verifier::validate_module;

#[path = "primitive_locals/ranking.rs"]
mod ranking;

fn scalar(ordinal: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(ordinal),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    }
}

fn constant(ordinal: u64, value: u128) -> Operation {
    Operation {
        id: operation_id(ordinal),
        result: OperationResult::Scalar(scalar(ordinal)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(value),
        },
    }
}

fn local_module() -> TerminalModule {
    let mut module = write_only_primitive_call_module();
    module.structural_types[0].shape = StructuralTypeShape::PrimitiveScalar(scalar(1).scalar_type);
    let caller = &mut module.machines[0];
    caller.structural_parameters.clear();
    caller.result = TerminalMachineResult::Scalar(scalar(5));
    caller.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(2),
        structural_type: structural_type_id(91),
    };
    caller.blocks[0].operations = vec![
        constant(1, 91),
        Operation {
            id: operation_id(2),
            result: OperationResult::Structural(StructuralOperationResult {
                place: place_id(91),
                structural_type: structural_type_id(91),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishPrimitiveLocal { value: value_id(1) },
        },
        Operation {
            id: operation_id(3),
            result: OperationResult::Scalar(scalar(3)),
            kind: OperationKind::CallStructuralScalar {
                callee: machine_id(92),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(91),
                    path: Vec::new(),
                    access: StructuralAccess::WriteOnlyBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
        Operation {
            id: operation_id(4),
            result: OperationResult::Scalar(scalar(4)),
            kind: OperationKind::PrimitiveScalarRead {
                source: place_id(91),
            },
        },
    ];
    caller.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(4),
        cleanup_actions: Vec::new(),
    };
    let callee = &mut module.machines[1];
    callee.result = TerminalMachineResult::Scalar(scalar(94));
    callee.blocks[0].operations[0] = constant(92, 0);
    callee.blocks[0].terminator = Terminator::Return {
        edge: edge_id(92),
        value: value_id(92),
        cleanup_actions: Vec::new(),
    };
    module
}

fn run(module: &TerminalModule, incremental: bool) -> (TerminalExecutionResult, u64) {
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_bundle(&ranking::proof(module)).unwrap();
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .unwrap();
    let mut meter = if incremental {
        TerminalFuelMeter::with_allowance(0)
    } else {
        TerminalFuelMeter::unbounded()
    };
    loop {
        match execution.resume(&mut meter).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(result) => {
                return (result, meter.usage().total_units());
            }
            other => panic!("unexpected primitive-local result: {other:?}"),
        }
    }
}

fn expected(value: u128) -> TerminalExecutionResult {
    TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    })
}

fn observe_local_identities(module: &TerminalModule) -> Vec<u64> {
    let mut module = module.clone();
    let mut parameter = module.machines[1].structural_parameters[0].clone();
    parameter.place = place_id(700);
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary_id(700),
        identity: "test::observe_primitive_loan".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![parameter],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
        crash_routes: Vec::new(),
    });
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(700),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_id(700),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(92),
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            }],
            completion_receipts: Vec::new(),
        },
    });
    #[derive(Default)]
    struct Identities(Vec<u64>);
    impl TerminalEffectHandler for Identities {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                structural_arguments,
                ..
            } = effect
            else {
                panic!("boundary observation");
            };
            self.0.push(structural_arguments[0].opaque_identity);
            Ok(())
        }
    }
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ranking::proof(&module)).unwrap();
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut identities = Identities::default();
    loop {
        match execution
            .resume_with_effect_handler(&mut meter, &mut identities)
            .unwrap()
        {
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            TerminalExecutionStatus::Complete(_) => return identities.0,
            other => panic!("unexpected primitive identity observation: {other:?}"),
        }
    }
}

#[test]
fn primitive_local_call_write_and_read_charge_once_across_suspension() {
    let module = local_module();
    assert_eq!(run(&module, false), (expected(0), 8));
    assert_eq!(run(&module, true), (expected(0), 8));
}

#[test]
fn primitive_local_direct_store_and_repeated_reads_share_backing() {
    let mut module = local_module();
    module.machines[0].blocks[0].operations[2] = Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::WriteOnlyPrimitiveStore {
            destination: place_id(91),
            value: value_id(1),
        },
    };
    module.machines.truncate(1);
    assert_eq!(run(&module, true).0, expected(91));
}

#[test]
fn primitive_local_missing_reordered_or_non_dominating_establishment_rejects() {
    let mut missing = local_module();
    missing.machines[0].blocks[0].operations.remove(1);
    assert!(validate_module(&missing).is_err());
    for use_position in [2, 3] {
        let mut reordered = local_module();
        reordered.machines[0].blocks[0]
            .operations
            .swap(1, use_position);
        assert!(matches!(
            validate_module(&reordered),
            Err(ModuleError::PrimitiveLocalNotAvailable { .. })
        ));
    }
    let mut branch = local_module();
    let caller = &mut branch.machines[0];
    let mut established = caller.blocks[0].clone();
    established.id = block_id(2);
    established.operations = vec![caller.blocks[0].operations[1].clone()];
    established.terminator = Terminator::Jump {
        edge: edge_id(3),
        target: block_id(3),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let mut join = caller.blocks[0].clone();
    join.id = block_id(3);
    join.operations.drain(..2);
    caller.blocks[0].operations.truncate(1);
    caller.blocks[0].operations.push(Operation {
        id: operation_id(6),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(6),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    let successor = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(6),
        when_true: successor(4, 2),
        when_false: successor(5, 3),
    };
    caller.blocks.extend([established, join]);
    assert!(matches!(
        validate_module(&branch),
        Err(ModuleError::PrimitiveLocalNotAvailable { .. })
    ));
}

#[test]
fn primitive_local_wrong_types_and_write_only_read_reject() {
    let mut input = local_module();
    input.machines[0].blocks[0].operations[0].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(1),
        scalar_type: ScalarType::Boolean,
    });
    input.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: false };
    assert!(matches!(
        validate_module(&input),
        Err(ModuleError::PrimitiveLocalValueTypeMismatch { .. })
    ));
    let mut output = local_module();
    output.machines[0].blocks[0].operations[3].result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(4),
        scalar_type: ScalarType::Boolean,
    });
    assert!(matches!(
        validate_module(&output),
        Err(ModuleError::InvalidPrimitiveScalarRead { .. })
    ));
    let mut read = local_module();
    read.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(95),
        result: OperationResult::Scalar(scalar(95)),
        kind: OperationKind::PrimitiveScalarRead {
            source: place_id(92),
        },
    });
    assert!(matches!(
        validate_module(&read),
        Err(ModuleError::InvalidPrimitiveScalarRead { .. })
    ));
    read.machines[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut read.machines[0].blocks[0].operations[2].kind
    else {
        panic!("call");
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(run(&read, true).0, expected(0));
}

#[test]
fn primitive_local_shared_call_is_not_affine_custody() {
    let mut module = local_module();
    module.machines[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    module.machines[1].blocks[0].operations = vec![Operation {
        id: operation_id(92),
        result: OperationResult::Scalar(scalar(92)),
        kind: OperationKind::PrimitiveScalarRead {
            source: place_id(92),
        },
    }];
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        panic!("call");
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert_eq!(run(&module, true).0, expected(91));
}

#[test]
fn primitive_local_overlapping_exclusive_actuals_and_owned_escape_reject() {
    let mut module = local_module();
    let mut parameter = module.machines[1].structural_parameters[0].clone();
    parameter.place = place_id(98);
    parameter.position = 1;
    module.machines[1].structural_parameters.push(parameter);
    module.machines[1]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(98),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[2].kind
    else {
        panic!("call");
    };
    structural_arguments.push(structural_arguments[0].clone());
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OverlappingExclusiveStructuralArguments { .. })
    ));

    let mut owned = local_module();
    owned.machines[1].structural_parameters[0].access = StructuralAccess::Owned;
    owned.machines[1].blocks[0].operations.truncate(1);
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut owned.machines[0].blocks[0].operations[2].kind
    else {
        panic!("call");
    };
    structural_arguments[0].access = StructuralAccess::Owned;
    assert!(validate_module(&owned).is_err());
}

#[test]
fn primitive_local_outer_storage_survives_nested_and_repeated_callee_activations() {
    let mut module = local_module();
    let mut wrapper = module.machines[0].clone();
    wrapper.id = machine_id(500);
    wrapper.contract.id = contract_id(500);
    wrapper.entry = block_id(500);
    wrapper.blocks[0].id = block_id(500);
    wrapper.result = TerminalMachineResult::Scalar(scalar(599));
    let mut parameter = module.machines[1].structural_parameters[0].clone();
    parameter.place = place_id(501);
    wrapper.structural_parameters = vec![parameter];
    wrapper.structural_places = vec![
        StructuralPlaceDeclaration {
            id: place_id(501),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(502),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(501),
                structural_type: structural_type_id(91),
            },
        },
    ];
    let mut establishment = wrapper.blocks[0].operations[1].clone();
    establishment.id = operation_id(501);
    establishment.kind = OperationKind::EstablishPrimitiveLocal {
        value: value_id(500),
    };
    let OperationResult::Structural(result) = &mut establishment.result else {
        panic!("local");
    };
    result.place = place_id(502);
    let mut call = wrapper.blocks[0].operations[2].clone();
    call.id = operation_id(502);
    call.result = OperationResult::Scalar(scalar(502));
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut call.kind
    else {
        panic!("call");
    };
    structural_arguments[0].place = place_id(502);
    wrapper.blocks[0].operations = vec![
        constant(500, 33),
        establishment,
        call,
        Operation {
            id: operation_id(503),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: place_id(501),
                value: value_id(500),
            },
        },
        Operation {
            id: operation_id(504),
            result: OperationResult::Scalar(scalar(504)),
            kind: OperationKind::PrimitiveScalarRead {
                source: place_id(502),
            },
        },
    ];
    wrapper.blocks[0].terminator = Terminator::Return {
        edge: edge_id(500),
        value: value_id(504),
        cleanup_actions: Vec::new(),
    };
    let caller = &mut module.machines[0];
    let OperationKind::CallStructuralScalar { callee, .. } =
        &mut caller.blocks[0].operations[2].kind
    else {
        panic!("call");
    };
    *callee = wrapper.id;
    let mut repeated = caller.blocks[0].operations[2].clone();
    repeated.id = operation_id(6);
    repeated.result = OperationResult::Scalar(scalar(6));
    caller.blocks[0].operations.insert(3, repeated);
    module.machines.push(wrapper);
    assert_eq!(run(&module, true).0, expected(33));
    assert_eq!(run(&module, false).0, expected(33));
    let identities = observe_local_identities(&module);
    assert_eq!(identities.len(), 2);
    assert_ne!(
        identities[0], identities[1],
        "repeated callee activations establish fresh roots"
    );
}

#[test]
fn primitive_local_loop_reestablishment_requires_no_affine_cleanup() {
    let mut module = ranking::module();
    let mut missing_rank_proof = ranking::proof(&module);
    missing_rank_proof.control_cycles.clear();
    assert!(verify_module(&module, &missing_rank_proof, &AdmissionProfile::default()).is_err());
    assert_eq!(run(&module, true), run(&module, false));
    assert_eq!(run(&module, false).0, expected(0));
    let identities = observe_local_identities(&module);
    assert_eq!(identities.len(), 3);
    assert_eq!(
        identities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    module.machines[0].blocks[0].operations.swap(1, 2);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::PrimitiveLocalNotAvailable { .. })
    ));
}

#[test]
fn primitive_local_formation_rejects_affine_custody_wrong_producer_and_duplicate_establishment() {
    let module = local_module();
    let mut affine = module.clone();
    let OperationResult::Structural(result) =
        &mut affine.machines[0].blocks[0].operations[1].result
    else {
        panic!("local");
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    assert!(validate_module(&affine).is_err());
    let mut wrong_producer = module.clone();
    wrong_producer.machines[0].structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(3),
        structural_type: structural_type_id(91),
    };
    assert!(validate_module(&wrong_producer).is_err());
    let mut duplicate = module;
    let establishment = duplicate.machines[0].blocks[0].operations[1].clone();
    duplicate.machines[0].blocks[0]
        .operations
        .insert(2, establishment);
    assert!(validate_module(&duplicate).is_err());
}
