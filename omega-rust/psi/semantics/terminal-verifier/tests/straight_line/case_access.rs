use super::*;
use terminal_psi::StructuralCaseSuccessorEdge;

fn membership_module(access: StructuralAccess) -> TerminalModule {
    let mut module = dispatch_module(access);
    let machine = &mut module.machines[0];
    machine.blocks.truncate(1);
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::StructuralCaseMembership {
            source: machine.structural_parameters[0].place,
            case: StructuralCaseId::new(1).unwrap(),
        },
    });
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(1).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

fn constructed_case_call_module(
    scalar_result: bool,
    multiplicity: StructuralMultiplicity,
) -> TerminalModule {
    let mut module = dispatch_module(StructuralAccess::Owned);
    let caller = &mut module.machines[0];
    let mut parameter = caller.structural_parameters.remove(0);
    parameter.multiplicity = multiplicity;
    caller.blocks.truncate(1);
    caller.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: OperationId::new(1).unwrap(),
        structural_type: parameter.structural_type,
    };
    caller.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case: StructuralCaseId::new(1).unwrap(),
            fields: Vec::new(),
        },
    }];
    caller.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(1).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    let mut callee = unit_module().machines.remove(0);
    callee.id = MachineId::new(901).unwrap();
    callee.contract.id = ContractId::new(901).unwrap();
    callee.structural_parameters = vec![parameter.clone()];
    callee.structural_parameters[0].place = PlaceId::new(3).unwrap();
    callee.structural_places = vec![StructuralPlaceDeclaration {
        id: PlaceId::new(3).unwrap(),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    callee.entry = BlockId::new(901).unwrap();
    callee.blocks[0].id = callee.entry;
    let discards = if multiplicity == StructuralMultiplicity::Affine {
        vec![PlaceId::new(3).unwrap()]
    } else {
        Vec::new()
    };
    let arguments = vec![StructuralArgument {
        place: parameter.place,
        path: Vec::new(),
        access: StructuralAccess::Owned,
    }];
    let (kind, result) = if scalar_result {
        let value = ValueDeclaration {
            id: ValueId::new(901).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        };
        callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: ValueId::new(902).unwrap(),
            ..value.clone()
        });
        callee.blocks[0].operations.push(Operation {
            static_reach_binding: None,
            id: OperationId::new(901).unwrap(),
            result: OperationResult::Scalar(value.clone()),
            kind: OperationKind::BooleanConstant { value: true },
        });
        // Affine disposal belongs to an explicit Unit edge, so the scalar-call
        // fixture is unrestricted; affine controls use the ordinary Unit call.
        assert!(discards.is_empty());
        callee.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(901).unwrap(),
            value: value.id,
            cleanup_actions: Vec::new(),
        };
        (
            OperationKind::CallStructuralScalar {
                callee: callee.id,
                arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
        )
    } else {
        callee.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(901).unwrap(),
            trivial_affine_discards: discards,
        };
        (
            OperationKind::CallUnit {
                callee: callee.id,
                arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
            OperationResult::Unit,
        )
    };
    caller.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: OperationId::new(2).unwrap(),
        result,
        kind,
    });
    module.machines.push(callee);
    module
}

#[test]
fn constructed_case_owned_arguments_retain_atomic_producer_custody() {
    for scalar in [false, true] {
        let module = constructed_case_call_module(scalar, StructuralMultiplicity::Unrestricted);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("fresh case reaches ordinary owned call");
    }
    let module = constructed_case_call_module(false, StructuralMultiplicity::Affine);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("affine case transfers once and is disposed by callee");
}

#[test]
fn constructed_case_owned_call_requires_prior_dominating_establishment() {
    for scalar in [false, true] {
        let mut module = constructed_case_call_module(scalar, StructuralMultiplicity::Unrestricted);
        module.machines[0].blocks[0].operations.swap(0, 1);
        assert_eq!(
            validate_module(&module).err(),
            Some(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: OperationId::new(2).unwrap(),
                place: PlaceId::new(1).unwrap()
            })
        );
    }
}

#[test]
fn constructed_case_shared_calls_preserve_copy_and_affine_owners() {
    for scalar in [false, true] {
        for multiplicity in [
            StructuralMultiplicity::Unrestricted,
            StructuralMultiplicity::Affine,
        ] {
            let mut module =
                constructed_case_call_module(scalar, StructuralMultiplicity::Unrestricted);
            module.machines[1].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            module.machines[1].structural_parameters[0].multiplicity = multiplicity;
            let caller = &mut module.machines[0];
            let OperationResult::Structural(result) = &mut caller.blocks[0].operations[0].result
            else {
                unreachable!()
            };
            result.multiplicity = multiplicity;
            match &mut caller.blocks[0].operations[1].kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => structural_arguments[0].access = StructuralAccess::SharedBorrow,
                _ => unreachable!(),
            }
            if multiplicity == StructuralMultiplicity::Affine {
                let Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } = &mut caller.blocks[0].terminator
                else {
                    unreachable!()
                };
                trivial_affine_discards.push(PlaceId::new(1).unwrap());
            }
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default(),
            )
            .unwrap_or_else(|error| {
                panic!("shared case scalar={scalar} multiplicity={multiplicity:?}: {error:?}")
            });
            module.machines[0].blocks[0].operations.swap(0, 1);
            assert_eq!(
                validate_module(&module).err(),
                Some(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: OperationId::new(2).unwrap(),
                    place: PlaceId::new(1).unwrap()
                })
            );
        }
    }
}

#[test]
fn constructed_case_copy_and_shared_actual_can_use_the_same_value() {
    for scalar in [false, true] {
        let mut module = constructed_case_call_module(scalar, StructuralMultiplicity::Unrestricted);
        let callee = &mut module.machines[1];
        let mut shared = callee.structural_parameters[0].clone();
        shared.place = PlaceId::new(4).unwrap();
        shared.position = 1;
        shared.access = StructuralAccess::SharedBorrow;
        callee.structural_parameters.push(shared);
        callee.structural_places.push(StructuralPlaceDeclaration {
            id: PlaceId::new(4).unwrap(),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
        match &mut module.machines[0].blocks[0].operations[1].kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => {
                structural_arguments.push(StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                });
            }
            _ => unreachable!(),
        }
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("copy plus shared scalar={scalar}: {error:?}"));
        module.machines[1].structural_parameters[1].access = StructuralAccess::MutableBorrow;
        match &mut module.machines[0].blocks[0].operations[1].kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            }
            | OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => {
                structural_arguments[1].access = StructuralAccess::MutableBorrow;
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&module).is_err(),
            "copy plus mutable is not admitted by shared case reads"
        );
    }
}

#[test]
fn constructed_case_sibling_establishment_cannot_authorize_owned_call() {
    let mut module = constructed_case_call_module(false, StructuralMultiplicity::Unrestricted);
    let machine = &mut module.machines[0];
    let mut operations = std::mem::take(&mut machine.blocks[0].operations);
    let call = operations.pop().unwrap();
    let construction = operations.pop().unwrap();
    machine.parameters.push(ValueDeclaration {
        id: ValueId::new(10).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(10).unwrap(),
        when_true: SuccessorEdge {
            edge: EdgeId::new(10).unwrap(),
            target: BlockId::new(10).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: EdgeId::new(11).unwrap(),
            target: BlockId::new(11).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    for (identity, operation) in [(10, construction), (11, call)] {
        machine.blocks.push(Block {
            id: BlockId::new(identity).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(identity + 10).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: OperationId::new(2).unwrap(),
            place: PlaceId::new(1).unwrap()
        })
    );
}

#[test]
fn constructed_affine_case_cannot_be_owned_by_two_calls() {
    let mut module = constructed_case_call_module(false, StructuralMultiplicity::Affine);
    let mut second = module.machines[0].blocks[0].operations[1].clone();
    second.id = OperationId::new(3).unwrap();
    module.machines[0].blocks[0].operations.push(second);
    assert!(validate_module(&module).is_err());
}

#[test]
fn constructed_case_owned_call_rejects_forged_producer_or_carrier() {
    for mutation in 0..2 {
        let mut module = constructed_case_call_module(false, StructuralMultiplicity::Unrestricted);
        if mutation == 1 {
            let mut foreign = module.structural_types[0].clone();
            foreign.id = StructuralTypeId::new(2).unwrap();
            foreign.identity = "ForeignChoice".into();
            let StructuralTypeShape::Sum { cases } = &mut foreign.shape else {
                unreachable!()
            };
            for (position, case) in cases.iter_mut().enumerate() {
                case.id = StructuralCaseId::new(3 + position as u64).unwrap();
            }
            module.structural_types.push(foreign);
        }
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = &mut module.machines[0].structural_places[0].kind
        else {
            unreachable!()
        };
        if mutation == 0 {
            *producer = OperationId::new(2).unwrap();
        } else {
            *structural_type = StructuralTypeId::new(2).unwrap();
        }
        assert!(
            validate_module(&module).is_err(),
            "forged case source {mutation}"
        );
    }
}

#[test]
fn case_membership_observes_each_readable_parameter_without_consumption() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let module = membership_module(access);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("readable membership verifies independently");
    }
}

#[test]
fn case_membership_rejects_write_only_access_even_with_a_refinement() {
    let mut module = membership_module(StructuralAccess::WriteOnlyBorrow);
    let source = module.machines[0].structural_parameters[0].place;
    for refined in [false, true] {
        if refined {
            module.machines[0]
                .contract
                .requires
                .push(Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(source, Vec::new()),
                    case: StructuralCaseId::new(1).unwrap(),
                });
        }
        assert_eq!(
            validate_module(&module).err(),
            Some(ModuleError::StructuralObservationRequiresReadableAccess {
                operation: OperationId::new(1).unwrap(),
                source,
            }),
        );
    }
}

#[test]
fn case_membership_rejects_same_named_foreign_case() {
    let mut module = membership_module(StructuralAccess::SharedBorrow);
    let mut foreign = module.structural_types[0].clone();
    foreign.id = StructuralTypeId::new(2).unwrap();
    foreign.identity = "foreign::Choice".to_owned();
    let StructuralTypeShape::Sum { cases } = &mut foreign.shape else {
        unreachable!()
    };
    cases[0].id = StructuralCaseId::new(3).unwrap();
    cases[1].id = StructuralCaseId::new(4).unwrap();
    module.structural_types.push(foreign);
    let OperationKind::StructuralCaseMembership { case, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *case = StructuralCaseId::new(3).unwrap();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidStructuralCaseObservation { .. }),
    ));
}

pub(super) fn dispatch_module(access: StructuralAccess) -> TerminalModule {
    let mut module = unit_module();
    let structural_type = StructuralTypeId::new(1).expect("sum type");
    let source = PlaceId::new(1).expect("source place");
    let cases = [
        StructuralCaseId::new(1).expect("first case"),
        StructuralCaseId::new(2).expect("second case"),
    ];
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Choice".to_owned(),
        shape: StructuralTypeShape::Sum {
            cases: cases
                .into_iter()
                .enumerate()
                .map(|(position, case)| StructuralCaseDeclaration {
                    id: case,
                    identity: format!("Case{position}"),
                    fields: Vec::new(),
                })
                .collect(),
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            is_self: false,
            structural_type,
            access,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: source,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    machine.blocks[0].terminator = Terminator::StructuralCase {
        source,
        cases: cases
            .into_iter()
            .enumerate()
            .map(|(position, case)| StructuralCaseSuccessorEdge {
                edge: EdgeId::new(10 + position as u64).expect("dispatch edge"),
                target: BlockId::new(10 + position as u64).expect("case block"),
                case,
                payload_fields: Vec::new(),
                trivial_affine_discards: Vec::new(),
            })
            .collect(),
    };
    for position in 0..2 {
        machine.blocks.push(Block {
            id: BlockId::new(10 + position).expect("case block"),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(20 + position).expect("return edge"),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    module
}

#[test]
fn readable_parameters_allow_fresh_structural_case_dispatch() {
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let module = dispatch_module(access);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("readable direct sum dispatch independently verifies");
    }
}

#[test]
fn write_only_parameter_cannot_drive_fresh_structural_case_dispatch() {
    let module = dispatch_module(StructuralAccess::WriteOnlyBorrow);
    assert!(module.machines[0].contract.requires.is_empty());
    assert!(module.machines[0].contract.crash_routes.is_empty());
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::StructuralCaseRequiresReadableAccess {
            machine: module.machines[0].id,
            block: module.machines[0].entry,
            source: module.machines[0].structural_parameters[0].place,
        }),
        "write-only tag dispatch must reject without relying on any contract predicate"
    );
}

#[test]
fn owned_operation_result_keeps_structural_case_dispatch() {
    let mut module = dispatch_module(StructuralAccess::Owned);
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters.remove(0);
    let operation = OperationId::new(1).expect("case producer");
    machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: operation,
        structural_type: source.structural_type,
    };
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: source.place,
            structural_type: source.structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case: StructuralCaseId::new(1).expect("first case"),
            fields: vec![],
        },
    });
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("owned constructed value retains tag observation");
}

#[test]
fn supplied_case_refinement_does_not_grant_executable_tag_access() {
    let mut module = dispatch_module(StructuralAccess::WriteOnlyBorrow);
    let machine = &mut module.machines[0];
    let source = machine.structural_parameters[0].place;
    machine
        .contract
        .requires
        .push(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(source, Vec::new()),
            case: StructuralCaseId::new(1).expect("supplied case"),
        });
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::StructuralCaseRequiresReadableAccess {
            machine: module.machines[0].id,
            block: module.machines[0].entry,
            source,
        }),
    );
}
