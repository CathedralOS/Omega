use super::{
    AdmissionProfile, Block, BlockId, ContractId, EdgeId, MachineId, ModuleError, Operation,
    OperationId, OperationKind, OperationResult, PlaceId, ProofBundle, Proposition, ScalarType,
    StructuralAccess, StructuralArgument, StructuralCaseDeclaration, StructuralCaseId,
    StructuralCaseSubject, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, SuccessorEdge,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, ValueId, unit_module,
    validate_module, verify_module,
};
use semantic_vocabulary::StructuralFieldId;
use terminal_psi::{
    RecordFieldInitializer, RecordFieldValue, StructuralCaseSuccessorEdge,
    StructuralFieldDeclaration, StructuralFieldType,
};

fn membership_module(access: StructuralAccess) -> TerminalModule {
    let mut module = dispatch_module(access);
    let machine = &mut module.machines[0];
    machine.blocks.truncate(1);
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::StructuralCaseMembership {
            path: Vec::new(),
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
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
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
            ..value
        });
        callee.blocks[0].operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(901).unwrap(),
            result: OperationResult::Scalar(value),
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
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
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
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
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
        suspension_crossing: None,
        id: OperationId::new(2).unwrap(),
        result,
        kind,
    });
    module.machines.push(callee);
    module
}

/// Keep the same caller/callee contract, but make the selected case contain
/// another case. The inner value establishes before the outer atomic producer;
/// only the outer value is supplied to the ordinary call.
fn nested_case_call_module(
    scalar_result: bool,
    multiplicity: StructuralMultiplicity,
) -> TerminalModule {
    let mut module = constructed_case_call_module(scalar_result, multiplicity);
    let child_type = StructuralTypeId::new(10).unwrap();
    let child_case = StructuralCaseId::new(10).unwrap();
    let child_place = PlaceId::new(10).unwrap();
    let child_operation = OperationId::new(10).unwrap();
    let field = StructuralFieldId::new(10).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: child_type,
        identity: "NestedChoice".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![StructuralCaseDeclaration {
                id: child_case,
                identity: "Present".into(),
                fields: Vec::new(),
            }],
        },
    });
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    cases[0].fields.push(StructuralFieldDeclaration {
        id: field,
        identity: "nested".into(),
        field_type: StructuralFieldType::Structural(child_type),
        relevance: terminal_psi::BindingRelevance::Relevant,
    });
    let caller = &mut module.machines[0];
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: child_place,
        kind: StructuralPlaceKind::OperationResult {
            producer: child_operation,
            structural_type: child_type,
        },
    });
    caller.blocks[0].operations[0].kind = OperationKind::EstablishStructuralCase {
        result_case: StructuralCaseId::new(1).unwrap(),
        fields: vec![RecordFieldInitializer {
            field,
            value: RecordFieldValue::Structural(StructuralArgument {
                place: child_place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }),
        }],
    };
    caller.blocks[0].operations.insert(
        0,
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: child_operation,
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: child_place,
                structural_type: child_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case: child_case,
                fields: Vec::new(),
            },
        },
    );
    module
}

#[test]
fn nested_case_owned_arguments_retain_atomic_producer_custody() {
    for (scalar, multiplicity) in [
        (false, StructuralMultiplicity::Unrestricted),
        (true, StructuralMultiplicity::Unrestricted),
        (false, StructuralMultiplicity::Affine),
    ] {
        verify_module(
            &nested_case_call_module(scalar, multiplicity),
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| {
            panic!("nested case scalar={scalar} multiplicity={multiplicity:?}: {error:?}")
        });
    }
}

#[test]
fn nested_case_copy_actuals_allow_copies_and_shared_observations_only() {
    for access in [StructuralAccess::Owned, StructuralAccess::SharedBorrow] {
        let mut module = nested_case_call_module(false, StructuralMultiplicity::Unrestricted);
        let callee = &mut module.machines[1];
        let mut parameter = callee.structural_parameters[0].clone();
        parameter.place = PlaceId::new(4).unwrap();
        parameter.position = 1;
        parameter.access = access;
        callee.structural_parameters.push(parameter);
        callee.structural_places.push(StructuralPlaceDeclaration {
            id: PlaceId::new(4).unwrap(),
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        structural_arguments.push(StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            access,
        });
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("nested copy plus {access:?}: {error:?}"));
        module.machines[1].structural_parameters[1].access = StructuralAccess::MutableBorrow;
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        structural_arguments[1].access = StructuralAccess::MutableBorrow;
        assert!(
            validate_module(&module).is_err(),
            "copy cannot alias a mutable loan"
        );
    }
}

#[test]
fn nested_case_call_requires_prior_producers_and_exact_carrier() {
    let valid = nested_case_call_module(false, StructuralMultiplicity::Unrestricted);
    for (first, second, operation, place) in [(1, 2, 2, 1), (0, 1, 1, 10)] {
        let mut module = valid.clone();
        module.machines[0].blocks[0].operations.swap(first, second);
        assert_eq!(
            validate_module(&module).err(),
            Some(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: OperationId::new(operation).unwrap(),
                place: PlaceId::new(place).unwrap(),
            }),
        );
    }
    for forged_producer in [false, true] {
        let mut module = valid.clone();
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = &mut module.machines[0].structural_places[0].kind
        else {
            unreachable!()
        };
        if forged_producer {
            *producer = OperationId::new(2).unwrap();
        } else {
            *structural_type = StructuralTypeId::new(10).unwrap();
        }
        assert!(
            validate_module(&module).is_err(),
            "nested producer/carrier forgery"
        );
    }
}

#[test]
fn nested_affine_case_cannot_be_owned_by_two_calls() {
    let mut module = nested_case_call_module(false, StructuralMultiplicity::Affine);
    let mut second = module.machines[0].blocks[0].operations[2].clone();
    second.id = OperationId::new(3).unwrap();
    module.machines[0].blocks[0].operations.push(second);
    assert_eq!(
        validate_module(&module).err(),
        Some(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: OperationId::new(3).unwrap(),
            place: PlaceId::new(1).unwrap(),
        }),
    );
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
    for module in [
        constructed_case_call_module(false, StructuralMultiplicity::Unrestricted),
        nested_case_call_module(false, StructuralMultiplicity::Unrestricted),
    ] {
        assert_sibling_establishment_cannot_authorize_call(module);
    }
}

fn assert_sibling_establishment_cannot_authorize_call(mut module: TerminalModule) {
    let machine = &mut module.machines[0];
    let mut operations = std::mem::take(&mut machine.blocks[0].operations);
    let call = operations.pop().unwrap();
    let construction = operations.pop().unwrap();
    // A nested payload may dominate both siblings; that does not establish
    // the enclosing case on the sibling which calls with it.
    machine.blocks[0].operations = operations;
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
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: EdgeId::new(11).unwrap(),
            target: BlockId::new(11).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    for (identity, operation) in [(10, construction), (11, call)] {
        machine.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
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

/// A mixed `Event { consumed: bool; case Nothing; case Treasure(gold: i32) }`
/// established from two constants. Common and payload field identities live in
/// separate namespaces and both use id 1 here, so only the roster position
/// (common fields first, then the selected payload) names each declaration.
fn mixed_case_module(
    roster: impl FnOnce(ValueId, ValueId) -> Vec<RecordFieldInitializer>,
) -> TerminalModule {
    let mut module = unit_module();
    let structural_type = StructuralTypeId::new(1).expect("mixed type");
    let place = PlaceId::new(1).expect("case place");
    let field = StructuralFieldId::new(1).expect("field id");
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32");
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Event".to_owned(),
        shape: StructuralTypeShape::Mixed {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: "consumed".to_owned(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
            cases: vec![
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).expect("first case"),
                    identity: "Nothing".to_owned(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).expect("second case"),
                    identity: "Treasure".to_owned(),
                    fields: vec![StructuralFieldDeclaration {
                        id: field,
                        identity: "gold".to_owned(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
                    }],
                },
            ],
        },
    });
    let flag = ValueId::new(1).expect("flag");
    let gold = ValueId::new(2).expect("gold");
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(3).expect("establishment"),
            structural_type,
        },
    });
    let scalar = |id: u64, value: ValueId, scalar_type, kind| Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(id).expect("scalar operation"),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value,
            scalar_type,
            qualifications: Default::default(),
        }),
        kind,
    };
    machine.blocks[0].operations = vec![
        scalar(
            1,
            flag,
            ScalarType::Boolean,
            OperationKind::BooleanConstant { value: true },
        ),
        scalar(
            2,
            gold,
            ScalarType::Integer(integer),
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Signed(50),
            },
        ),
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(3).expect("establishment"),
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishStructuralCase {
                result_case: StructuralCaseId::new(2).expect("second case"),
                fields: roster(flag, gold),
            },
        },
    ];
    module
}

fn scalar_initializer(value: ValueId) -> RecordFieldInitializer {
    RecordFieldInitializer {
        field: StructuralFieldId::new(1).expect("field id"),
        value: RecordFieldValue::Scalar {
            value,
            range_obligation: None,
        },
    }
}

#[test]
fn mixed_case_establishment_binds_common_fields_then_the_payload() {
    let module =
        mixed_case_module(|flag, gold| vec![scalar_initializer(flag), scalar_initializer(gold)]);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a mixed case binds its common field, then its payload");
}

#[test]
fn mixed_case_establishment_rejects_a_misplaced_or_incomplete_roster() {
    let rosters: [fn(ValueId, ValueId) -> Vec<RecordFieldInitializer>; 4] = [
        // The payload value in the common position and vice versa.
        |flag, gold| vec![scalar_initializer(gold), scalar_initializer(flag)],
        // The payload alone, as a closed sum would bind it.
        |_, gold| vec![scalar_initializer(gold)],
        // The common field alone.
        |flag, _| vec![scalar_initializer(flag)],
        // A second payload entry beyond the declared roster.
        |flag, gold| {
            vec![
                scalar_initializer(flag),
                scalar_initializer(gold),
                scalar_initializer(gold),
            ]
        },
    ];
    for (index, roster) in rosters.into_iter().enumerate() {
        let module = mixed_case_module(roster);
        assert!(
            validate_module(&module).is_err(),
            "roster mutation {index} must not establish the mixed case"
        );
    }
}

#[test]
fn mixed_case_establishment_refuses_a_structural_common_field() {
    let mut module =
        mixed_case_module(|flag, gold| vec![scalar_initializer(flag), scalar_initializer(gold)]);
    let StructuralTypeShape::Mixed { fields, .. } = &mut module.structural_types[0].shape else {
        panic!("mixed fixture");
    };
    let child = StructuralTypeId::new(2).expect("child record");
    fields[0].field_type = StructuralFieldType::Structural(child);
    module.structural_types.push(StructuralTypeDeclaration {
        id: child,
        identity: "Flags".to_owned(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).expect("flag field"),
                identity: "set".to_owned(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
    });
    let error = validate_module(&module).expect_err("a structural common field stays refused");
    assert!(
        matches!(error, ModuleError::StructuralCaseRequiresSum { .. }),
        "{error:?}"
    );
}

/// Add an erased `proof` member after the `Treasure` payload's `gold`. It
/// carries its declaration only, so the roster still binds `consumed`, then
/// `gold`, and takes no position for `proof`.
fn with_erased_payload_member(mut module: TerminalModule) -> TerminalModule {
    let StructuralTypeShape::Mixed { cases, .. } = &mut module.structural_types[0].shape else {
        panic!("mixed fixture");
    };
    cases[1].fields.push(StructuralFieldDeclaration {
        id: StructuralFieldId::new(2).expect("proof field"),
        identity: "proof".to_owned(),
        relevance: terminal_psi::BindingRelevance::Erased,
        field_type: StructuralFieldType::Erased {
            type_identity: "Evidence".to_owned(),
        },
    });
    module
}

#[test]
fn case_establishment_skips_an_erased_payload_member() {
    let module = with_erased_payload_member(mixed_case_module(|flag, gold| {
        vec![scalar_initializer(flag), scalar_initializer(gold)]
    }));
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("an erased payload member takes no roster position");
}

#[test]
fn case_establishment_refuses_a_binding_for_an_erased_payload_member() {
    // Binding the erased member as if it were a runtime field overruns the
    // runtime roster.
    let module = with_erased_payload_member(mixed_case_module(|flag, gold| {
        vec![
            scalar_initializer(flag),
            scalar_initializer(gold),
            RecordFieldInitializer {
                field: StructuralFieldId::new(2).expect("proof field"),
                value: RecordFieldValue::Scalar {
                    value: gold,
                    range_obligation: None,
                },
            },
        ]
    }));
    assert!(validate_module(&module).is_err());
    // An erased relevance over a runtime field type is not a proof member and
    // stays unconstructible.
    let mut mismatched = with_erased_payload_member(mixed_case_module(|flag, gold| {
        vec![scalar_initializer(flag), scalar_initializer(gold)]
    }));
    let StructuralTypeShape::Mixed { cases, .. } = &mut mismatched.structural_types[0].shape else {
        panic!("mixed fixture");
    };
    cases[1].fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
    assert!(validate_module(&mismatched).is_err());
}

fn borrowed_case_payload_module(reference: bool) -> TerminalModule {
    use terminal_psi::StructuralPathSegment;
    let mut module = dispatch_module(StructuralAccess::SharedBorrow);
    let payload = StructuralTypeId::new(2).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: payload,
        identity: "Payload".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
    });
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        panic!("sum");
    };
    // Equal payload types make wrong-case rejection depend on current case
    // custody, rather than an incidental carrier mismatch.
    for case in cases {
        case.fields.push(StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: "payload".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(payload),
        });
    }
    let source = StructuralArgument {
        place: PlaceId::new(1).unwrap(),
        access: StructuralAccess::SharedBorrow,
        path: vec![
            StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap()),
            StructuralPathSegment::Field("payload".into()),
        ],
    };
    if reference {
        let reference_type = StructuralTypeId::new(3).unwrap();
        module.structural_types.push(StructuralTypeDeclaration {
            id: reference_type,
            identity: "SharedPayload".into(),
            shape: StructuralTypeShape::Reference {
                referent: payload,
                access: StructuralAccess::SharedBorrow,
            },
        });
        let result = PlaceId::new(2).unwrap();
        module.machines[0]
            .structural_places
            .push(StructuralPlaceDeclaration {
                id: result,
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(1).unwrap(),
                    structural_type: reference_type,
                },
            });
        module.machines[0].blocks[1].operations.extend([
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(1).unwrap(),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: result,
                    structural_type: reference_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                    qualification_establishments: Vec::new(),
                }),
                kind: OperationKind::EstablishReference { source },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(2).unwrap(),
                result: OperationResult::Unit,
                kind: OperationKind::ReleaseReference { source: result },
            },
        ]);
    } else {
        let mut callee = unit_module().machines.remove(0);
        callee.id = MachineId::new(901).unwrap();
        callee.contract.id = ContractId::new(901).unwrap();
        callee.entry = BlockId::new(901).unwrap();
        callee.blocks[0].id = callee.entry;
        callee.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(901).unwrap(),
            trivial_affine_discards: Vec::new(),
        };
        callee.structural_parameters = vec![StructuralParameterDeclaration {
            place: PlaceId::new(901).unwrap(),
            position: 0,
            is_self: false,
            structural_type: payload,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        callee.structural_places = vec![StructuralPlaceDeclaration {
            id: PlaceId::new(901).unwrap(),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        module.machines[0].blocks[1].operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: callee.id,
                arguments: Vec::new(),
                structural_arguments: vec![source],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        module.machines.push(callee);
    }
    module
}

#[test]
fn borrowed_case_payload_requires_current_membership_for_call_and_reference() {
    use terminal_psi::StructuralPathSegment;
    for reference in [false, true] {
        let module = borrowed_case_payload_module(reference);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("valid projected borrow reference={reference}: {error:?}"));
        for mutation in 0..6 {
            let mut invalid = module.clone();
            let operation = &mut invalid.machines[0].blocks[1].operations[0];
            let source = match &mut operation.kind {
                OperationKind::EstablishReference { source } => source,
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => &mut structural_arguments[0],
                _ => panic!("borrow"),
            };
            match mutation {
                0 => {
                    source.path[0] = StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap())
                }
                1 => source.path[1] = StructuralPathSegment::Field("missing".into()),
                2 => source.path.truncate(1),
                3 => source.access = StructuralAccess::MutableBorrow,
                4 => {
                    // Move the use before dispatch; later membership must not
                    // retroactively authorize either call or loan formation.
                    let operations = std::mem::take(&mut invalid.machines[0].blocks[1].operations);
                    invalid.machines[0].blocks[0].operations = operations;
                }
                _ => {
                    source.place = PlaceId::new(3).unwrap();
                    let mut parameter = invalid.machines[0].structural_parameters[0].clone();
                    parameter.place = PlaceId::new(3).unwrap();
                    parameter.position = 1;
                    invalid.machines[0].structural_parameters.push(parameter);
                    invalid.machines[0]
                        .structural_places
                        .push(StructuralPlaceDeclaration {
                            id: PlaceId::new(3).unwrap(),
                            kind: StructuralPlaceKind::Parameter {
                                position: 1,
                                is_self: false,
                            },
                        });
                }
            }
            assert!(
                verify_module(
                    &invalid,
                    &ProofBundle::default(),
                    &AdmissionProfile::default()
                )
                .is_err(),
                "forged case borrow reference={reference}, mutation={mutation}"
            );
        }
    }
}

#[test]
fn borrowed_case_payload_membership_expires_after_mutable_call() {
    let mut module = borrowed_case_payload_module(false);
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let mut mutation = module.machines[1].clone();
    mutation.id = MachineId::new(902).unwrap();
    mutation.contract.id = ContractId::new(902).unwrap();
    mutation.entry = BlockId::new(902).unwrap();
    mutation.blocks[0].id = mutation.entry;
    mutation.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(902).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    mutation.structural_parameters[0].place = PlaceId::new(902).unwrap();
    mutation.structural_places[0].id = PlaceId::new(902).unwrap();
    mutation.structural_parameters[0].structural_type = StructuralTypeId::new(1).unwrap();
    mutation.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let mut call = module.machines[0].blocks[1].operations[0].clone();
    call.id = OperationId::new(2).unwrap();
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        ..
    } = &mut call.kind
    else {
        panic!("call");
    };
    *callee = mutation.id;
    structural_arguments[0].path.clear();
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    module.machines.push(mutation);
    module.machines[0].blocks[1].operations.push(call);
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the projected call is before the potentially mutating call");
    module.machines[0].blocks[1].operations.swap(0, 1);
    validate_module(&module).expect("operation shapes and borrowed access remain valid");
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::Module(
            ModuleError::InvalidStructuralCaseObservation { .. }
        ))
    ));
    replace_payload_call_with_successor(&mut module);
    validate_module(&module).expect("the successor loan retains valid geometry and access");
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::Module(
            ModuleError::InvalidStructuralSuccessorArgument { .. }
        ))
    ));
}

fn borrowed_case_payload_successor_module() -> TerminalModule {
    let mut module = borrowed_case_payload_module(false);
    replace_payload_call_with_successor(&mut module);
    module
}

fn replace_payload_call_with_successor(module: &mut TerminalModule) {
    let callee = module.machines.remove(1);
    let machine = &mut module.machines[0];
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = machine.blocks[1]
        .operations
        .pop()
        .expect("payload call")
        .kind
    else {
        panic!("payload call");
    };
    let mut target = callee.blocks[0].clone();
    target.structural_parameters = callee.structural_parameters;
    for parameter in &target.structural_parameters {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::BlockParameter {
                block: target.id,
                position: parameter.position,
            },
        });
    }
    machine.blocks[1].terminator = Terminator::Jump {
        edge: EdgeId::new(30).unwrap(),
        target: target.id,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(target);
}

#[test]
fn borrowed_case_payload_successor_requires_the_selected_case() {
    let mut module = borrowed_case_payload_successor_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a dominating case dispatch authorizes its payload loan");
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[1].terminator
    else {
        panic!("payload successor");
    };
    structural_arguments[0].path[0] =
        terminal_psi::StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap());
    validate_module(&module).expect("both cases have the identical payload type");
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::Module(
            ModuleError::InvalidStructuralSuccessorArgument { .. }
        ))
    ));
}

#[test]
fn borrowed_case_payload_conditional_successor_checks_each_edge_truth() {
    let mut module = borrowed_case_payload_successor_module();
    let membership = membership_module(StructuralAccess::SharedBorrow)
        .machines
        .remove(0)
        .blocks
        .remove(0)
        .operations
        .remove(0);
    let machine = &mut module.machines[0];
    let Terminator::Jump {
        target,
        structural_arguments,
        ..
    } = machine.blocks.remove(1).terminator
    else {
        panic!("payload successor");
    };
    machine.blocks.remove(1); // Both arms now borrow their own selected payload.
    machine.blocks[0].operations.push(membership);
    let when_true = SuccessorEdge {
        edge: EdgeId::new(10).unwrap(),
        target,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments,
        trivial_affine_discards: Vec::new(),
    };
    let mut when_false = when_true.clone();
    when_false.edge = EdgeId::new(11).unwrap();
    when_false.structural_arguments[0].path[0] =
        terminal_psi::StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap());
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: ValueId::new(1).unwrap(),
        when_true,
        when_false,
    };
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the true edge proves membership and the false edge exhausts the other case");

    // Keeping the same membership observation but reversing its destinations
    // cannot authorize the inactive payload on either edge.
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        panic!("membership branch");
    };
    std::mem::swap(
        &mut when_true.structural_arguments,
        &mut when_false.structural_arguments,
    );
    validate_module(&module).expect("the forged operands retain identical types and access");
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(terminal_verifier::VerificationError::Module(
            ModuleError::InvalidStructuralSuccessorArgument { .. }
        ))
    ));
}
