use super::{
    AdmissionProfile, BindingRelevance, Block, FuelChargeSite, IntegerSign, IntegerType,
    IntegerValue, ModuleError, Operation, OperationKind, OperationResult, ProofBundle, ScalarType,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalInterpretError,
    TerminalMachineResult, TerminalModule, TerminalScalarValue, TerminalStructuralValue,
    Terminator, ValueDeclaration, block_id, contract_id, decode_module, edge_id, encode_module,
    encode_proof_section, interpret_terminal_artifact_measured, machine_id, operation_id,
    payloadless_case_module, place_id, structural_case_id, structural_field_id, structural_type_id,
    value_id,
};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::{TerminalStructuralCaseValue, TerminalStructuralInputs};

fn projected_membership_module(
    selected_index: u64,
    nested: bool,
    access: StructuralAccess,
    multiplicity: StructuralMultiplicity,
) -> TerminalModule {
    let mut module = payloadless_case_module();
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    cases.push(terminal_psi::StructuralCaseDeclaration {
        id: structural_case_id(2),
        identity: "Other".into(),
        fields: Vec::new(),
    });
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(2),
        identity: "test::Palette".into(),
        shape: StructuralTypeShape::FixedArray {
            element: structural_type_id(1),
            length: 2,
        },
    });
    if nested {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "test::Owner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "colors".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(2)),
                }],
            },
        });
    }
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: structural_type_id(if nested { 3 } else { 2 }),
        multiplicity,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(1),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    let boolean = |identity| ValueDeclaration {
        id: value_id(identity),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    };
    machine.result = TerminalMachineResult::Scalar(boolean(3));
    let mut path = Vec::new();
    if nested {
        path.push(StructuralPathSegment::Field("colors".into()));
    }
    path.push(StructuralPathSegment::FixedIndex(selected_index));
    machine.blocks[0].operations = (1..=2)
        .map(|identity| Operation {
            static_reach_binding: None,
            id: operation_id(identity),
            result: OperationResult::Scalar(boolean(identity)),
            kind: OperationKind::StructuralCaseMembership {
                source: place_id(1),
                path: path.clone(),
                case: structural_case_id(1),
            },
        })
        .collect();
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(2),
        cleanup_actions: if access == StructuralAccess::Owned
            && multiplicity == StructuralMultiplicity::Affine
        {
            vec![TerminalAffineCleanupAction::DiscardRoot(place_id(1))]
        } else {
            Vec::new()
        },
    };
    module
}

fn projected_cases(nested: bool) -> Vec<TerminalStructuralCaseValue> {
    (0..2)
        .map(|element_index| {
            let mut path = Vec::new();
            if nested {
                path.push(StructuralPathSegment::Field("colors".into()));
            }
            path.push(StructuralPathSegment::FixedIndex(element_index));
            TerminalStructuralCaseValue {
                argument_index: 0,
                path,
                case: structural_case_id(element_index + 1),
            }
        })
        .collect()
}

fn projected_execution(
    module: &TerminalModule,
    contents: &[TerminalStructuralCaseValue],
) -> Result<TerminalExecution, terminal_interpreter::TerminalArtifactInterpretError> {
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    TerminalExecution::start_artifact(
        &semantic,
        &encode_proof_section(module, &ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 77,
                structural_type: module.machines[0].structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            cases: contents,
            ..Default::default()
        },
    )
}

#[test]
fn projected_case_membership_preserves_distinct_array_elements_and_live_root() {
    for nested in [false, true] {
        for (access, multiplicity) in [
            (
                StructuralAccess::Owned,
                StructuralMultiplicity::Unrestricted,
            ),
            (StructuralAccess::Owned, StructuralMultiplicity::Affine),
            (
                StructuralAccess::SharedBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
            (
                StructuralAccess::MutableBorrow,
                StructuralMultiplicity::Unrestricted,
            ),
        ] {
            for selected_index in 0..2 {
                let module =
                    projected_membership_module(selected_index, nested, access, multiplicity);
                let mut execution = projected_execution(&module, &projected_cases(nested))
                    .expect("projected tag contents independently verify");
                let mut meter = TerminalFuelMeter::with_allowance(1);
                assert!(matches!(
                    execution
                        .resume(&mut meter, &mut AcceptTerminalEffects)
                        .unwrap(),
                    TerminalExecutionStatus::SponsorExhausted(_)
                ));
                assert_eq!(
                    execution.live_affine_frontier().count(),
                    usize::from(multiplicity == StructuralMultiplicity::Affine)
                );
                meter.replenish(2).unwrap();
                assert_eq!(
                    execution
                        .resume(&mut meter, &mut AcceptTerminalEffects)
                        .unwrap(),
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                        TerminalScalarValue::Boolean(selected_index == 0)
                    ))
                );
                assert_eq!(meter.usage().total_units(), 3);
            }
        }
    }
}

#[test]
fn projected_case_membership_rejects_invalid_path_type_and_access() {
    let module = projected_membership_module(
        1,
        true,
        StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    terminal_verifier::validate_module(&module).unwrap();
    for mutation in 0..7 {
        let mut changed = module.clone();
        match mutation {
            0 => {
                let OperationKind::StructuralCaseMembership { path, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                path[1] = StructuralPathSegment::FixedIndex(2);
            }
            1 => {
                let OperationKind::StructuralCaseMembership { path, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                path[0] = StructuralPathSegment::Field("unknown".into());
            }
            2 => {
                let StructuralTypeShape::Record { fields } = &mut changed.structural_types[2].shape
                else {
                    unreachable!()
                };
                fields[0].relevance = BindingRelevance::Erased;
            }
            3 => {
                let OperationKind::StructuralCaseMembership { path, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                path.pop();
            }
            4 => {
                changed.machines[0].structural_parameters[0].access =
                    StructuralAccess::WriteOnlyBorrow
            }
            5 => {
                let OperationKind::StructuralCaseMembership { source, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                *source = place_id(99);
            }
            _ => {
                let mut foreign = changed.structural_types[0].clone();
                foreign.id = structural_type_id(4);
                foreign.identity = "test::Foreign".into();
                let StructuralTypeShape::Sum { cases } = &mut foreign.shape else {
                    unreachable!()
                };
                cases[0].id = structural_case_id(3);
                cases[1].id = structural_case_id(4);
                changed.structural_types.push(foreign);
                let OperationKind::StructuralCaseMembership { case, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                *case = structural_case_id(3);
            }
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "tampered projection {mutation}"
        );
    }
    let mut changed = module.clone();
    let OperationKind::StructuralCaseMembership { path, .. } =
        &mut changed.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    path[1] = StructuralPathSegment::FixedIndex(0);
    assert_ne!(
        terminal_codec::semantic_fingerprint(&module).unwrap(),
        terminal_codec::semantic_fingerprint(&changed).unwrap()
    );
}

#[test]
fn projected_case_contents_reject_unknown_cases_alias_duplicates_and_bad_paths() {
    let module = projected_membership_module(
        1,
        true,
        StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    for mutation in 0..3 {
        let mut contents = projected_cases(true);
        match mutation {
            0 => contents[0].case = structural_case_id(99),
            1 => contents.push(contents[0].clone()),
            _ => contents[0].path[1] = StructuralPathSegment::FixedIndex(2),
        }
        assert!(projected_execution(&module, &contents).is_err());
    }

    let mut payloadful = module;
    let StructuralTypeShape::Sum { cases } = &mut payloadful.structural_types[0].shape else {
        unreachable!()
    };
    cases[0].fields.push(StructuralFieldDeclaration {
        id: structural_field_id(2),
        identity: "required_payload".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    assert!(projected_execution(&payloadful, &projected_cases(true)).is_err());
}

#[test]
fn projected_case_encoding_requires_the_extended_operation_format() {
    let module = projected_membership_module(
        1,
        false,
        StructuralAccess::SharedBorrow,
        StructuralMultiplicity::Unrestricted,
    );
    let mut semantic = encode_module(&module).unwrap();
    assert_eq!(&semantic[8..10], &100_u16.to_le_bytes());
    semantic[8..10].copy_from_slice(&96_u16.to_le_bytes());
    assert_eq!(
        decode_module(&semantic),
        Err(terminal_codec::CodecError::UnsupportedFormatMarker(96))
    );
}

#[test]
fn projected_case_membership_checks_nominal_type_through_host_aliases() {
    for foreign_type in [false, true] {
        let mut module = projected_membership_module(
            1,
            false,
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Unrestricted,
        );
        let mut foreign = module.structural_types[0].clone();
        foreign.id = structural_type_id(3);
        foreign.identity = "test::UnrelatedOutcome".into();
        let StructuralTypeShape::Sum { cases } = &mut foreign.shape else {
            unreachable!()
        };
        cases[0].id = structural_case_id(3);
        cases[1].id = structural_case_id(4);
        module.structural_types.push(foreign);
        let alias_type = structural_type_id(if foreign_type { 3 } else { 1 });
        let machine = &mut module.machines[0];
        let mut alias = machine.structural_parameters[0].clone();
        alias.place = place_id(2);
        alias.position = 1;
        alias.structural_type = alias_type;
        machine.structural_parameters.push(alias);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(2),
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
        machine.blocks[0].operations[1].kind = OperationKind::StructuralCaseMembership {
            source: place_id(2),
            path: Vec::new(),
            case: structural_case_id(if foreign_type { 3 } else { 1 }),
        };
        let mut execution = TerminalExecution::start_artifact(
            &encode_module(&module).unwrap(),
            &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[
                    TerminalStructuralValue {
                        opaque_identity: 77,
                        structural_type: structural_type_id(2),
                        qualifications: Vec::new(),
                        path: Vec::new(),
                    },
                    TerminalStructuralValue {
                        opaque_identity: 77,
                        structural_type: alias_type,
                        qualifications: Vec::new(),
                        path: vec![StructuralPathSegment::FixedIndex(1)],
                    },
                ],
                cases: &projected_cases(false),
                ..Default::default()
            },
        )
        .unwrap();
        let outcome = execution.resume(
            &mut TerminalFuelMeter::with_allowance(3),
            &mut AcceptTerminalEffects,
        );
        if foreign_type {
            assert_eq!(
                outcome,
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            );
        } else {
            assert_eq!(
                outcome,
                Ok(TerminalExecutionStatus::Complete(
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
                ))
            );
        }
    }
}

#[test]
fn projected_case_membership_calls_keep_the_original_root_and_selected_index() {
    for selected_index in 0..2 {
        let mut module = projected_membership_module(
            selected_index,
            true,
            StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Unrestricted,
        );
        let mut callee = module.machines[0].clone();
        callee.id = machine_id(2);
        callee.contract.id = contract_id(2);
        callee.entry = block_id(2);
        callee.structural_parameters[0].structural_type = structural_type_id(1);
        callee.structural_parameters[0].place = place_id(5);
        callee.structural_places[0].id = place_id(5);
        callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: value_id(11),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        });
        callee.blocks[0].id = block_id(2);
        callee.blocks[0].operations.truncate(1);
        callee.blocks[0].operations[0].id = operation_id(10);
        callee.blocks[0].operations[0]
            .result
            .scalar_mut()
            .unwrap()
            .id = value_id(10);
        callee.blocks[0].operations[0].kind = OperationKind::StructuralCaseMembership {
            source: place_id(5),
            path: Vec::new(),
            case: structural_case_id(1),
        };
        callee.blocks[0].terminator = Terminator::Return {
            edge: edge_id(2),
            value: value_id(10),
            cleanup_actions: Vec::new(),
        };
        for operation in &mut module.machines[0].blocks[0].operations {
            let OperationKind::StructuralCaseMembership { source, path, .. } = &operation.kind
            else {
                unreachable!()
            };
            operation.kind = OperationKind::CallStructuralScalar {
                erased_arguments: Vec::new(),
                callee: machine_id(2),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: *source,
                    path: path.clone(),
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            };
        }
        module.machines.push(callee);
        let mut execution = projected_execution(&module, &projected_cases(true)).unwrap();
        assert_eq!(
            execution
                .resume(
                    &mut TerminalFuelMeter::with_allowance(7),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Boolean(selected_index == 0)
            ))
        );
    }
}

#[test]
fn projected_case_membership_rejects_a_discarded_affine_root() {
    let mut module = projected_membership_module(
        1,
        true,
        StructuralAccess::Owned,
        StructuralMultiplicity::Affine,
    );
    let machine = &mut module.machines[0];
    let mut continuation = machine.blocks[0].clone();
    continuation.id = block_id(2);
    continuation.terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(2),
        cleanup_actions: Vec::new(),
    };
    machine.blocks[0].operations.clear();
    machine.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(1),
        target: block_id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: vec![place_id(1)],
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(continuation);
    assert!(matches!(terminal_verifier::validate_module(&module),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { operation, place })
            if operation == operation_id(1) && place == place_id(1)));
}

fn observed_constructor(
    active_case: u64,
    observed_case: u64,
    multiplicity: StructuralMultiplicity,
) -> TerminalModule {
    let mut module = payloadless_case_module();
    let StructuralTypeShape::Sum { cases } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    cases.push(terminal_psi::StructuralCaseDeclaration {
        id: structural_case_id(2),
        identity: "Some".into(),
        fields: vec![StructuralFieldDeclaration {
            id: structural_field_id(1),
            identity: "value".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            )),
        }],
    });
    let machine = &mut module.machines[0];
    machine.structural_places.truncate(1);
    machine.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::OperationResult {
        producer: operation_id(2),
        structural_type: structural_type_id(1),
    };
    let mut constructor = machine.blocks[0].operations.remove(0);
    constructor.id = operation_id(2);
    let OperationResult::Structural(result) = &mut constructor.result else {
        unreachable!()
    };
    result.multiplicity = multiplicity;
    constructor.kind = OperationKind::EstablishScalarCase {
        result_case: structural_case_id(active_case),
        fields: if active_case == 2 {
            vec![terminal_psi::ScalarCaseField {
                field: structural_field_id(1),
                value: value_id(1),
                range_obligation: None,
            }]
        } else {
            vec![]
        },
    };
    let boolean = |identity| ValueDeclaration {
        id: value_id(identity),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    };
    machine.result = TerminalMachineResult::Scalar(boolean(4));
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(1),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(37),
            },
        },
        constructor,
        Operation {
            static_reach_binding: None,
            id: operation_id(3),
            result: OperationResult::Scalar(boolean(2)),
            kind: OperationKind::StructuralCaseMembership {
                path: Vec::new(),
                source: place_id(1),
                case: structural_case_id(observed_case),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(4),
            result: OperationResult::Scalar(boolean(3)),
            kind: OperationKind::StructuralCaseMembership {
                path: Vec::new(),
                source: place_id(1),
                case: structural_case_id(observed_case),
            },
        },
    ];
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(2),
        cleanup_actions: if multiplicity == StructuralMultiplicity::Affine {
            vec![TerminalAffineCleanupAction::DiscardRoot(place_id(1))]
        } else {
            vec![]
        },
    };
    module
}

#[test]
fn case_membership_round_trips_both_tags_and_preserves_repeated_reads() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        for active in [1, 2] {
            for observed in [1, 2] {
                let module = observed_constructor(active, observed, multiplicity);
                let semantic = encode_module(&module).unwrap();
                let decoded = decode_module(&semantic).unwrap();
                assert_eq!(decoded, module);
                let measured = interpret_terminal_artifact_measured(
                    &semantic,
                    &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
                    &AdmissionProfile::default(),
                    &[],
                    TerminalStructuralInputs::default(),
                    &mut AcceptTerminalEffects,
                )
                .expect("case observation leaves the owner available for another read and cleanup");
                assert_eq!(
                    measured.value(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                        active == observed
                    ))
                );
                assert_eq!(measured.usage().total_units(), 5);
            }
        }
    }
}

#[test]
fn ordinary_case_calls_move_affine_and_preserve_copy_or_shared_payloads() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        for access in [StructuralAccess::Owned, StructuralAccess::SharedBorrow] {
            let mut module = observed_constructor(2, 2, multiplicity);
            let mut callee = module.machines[0].clone();
            callee.id = machine_id(2);
            callee.contract.id = contract_id(2);
            callee.entry = block_id(2);
            let result = ValueDeclaration {
                id: value_id(12),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            };
            callee.result = TerminalMachineResult::Scalar(result);
            callee.structural_parameters = vec![StructuralParameterDeclaration {
                place: place_id(5),
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: if access == StructuralAccess::Owned {
                    multiplicity
                } else {
                    StructuralMultiplicity::Unrestricted
                },
                access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }];
            callee.structural_places = vec![StructuralPlaceDeclaration {
                id: place_id(5),
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }];
            callee.blocks = vec![Block {
                erased_scalar_formals: Vec::new(),
                id: block_id(2),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(10),
                    static_reach_binding: None,
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value_id(10),
                        ..result
                    }),
                    kind: OperationKind::StructuralCaseMembership {
                        path: Vec::new(),
                        source: place_id(5),
                        case: structural_case_id(2),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(2),
                    value: value_id(10),
                    cleanup_actions: if access == StructuralAccess::Owned
                        && multiplicity == StructuralMultiplicity::Affine
                    {
                        vec![TerminalAffineCleanupAction::DiscardRoot(place_id(5))]
                    } else {
                        Vec::new()
                    },
                },
            }];
            let caller = &mut module.machines[0];
            caller.blocks[0].operations[2].kind = OperationKind::CallStructuralScalar {
                erased_arguments: Vec::new(),
                callee: machine_id(2),
                arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument {
                    place: place_id(1),
                    path: Vec::new(),
                    access,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            };
            if access == StructuralAccess::Owned && multiplicity == StructuralMultiplicity::Affine {
                caller.blocks[0].operations.truncate(3);
                if let Terminator::Return {
                    cleanup_actions, ..
                } = &mut caller.blocks[0].terminator
                {
                    cleanup_actions.clear();
                }
            }
            module.machines.push(callee);
            let measured = interpret_terminal_artifact_measured(
                &encode_module(&module).expect("complete case call encodes"),
                &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
                &AdmissionProfile::default(),
                &[],
                TerminalStructuralInputs::default(),
                &mut AcceptTerminalEffects,
            )
            .expect("case payload survives exact call transfer");
            assert_eq!(
                measured.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
            );
            assert_eq!(
                measured
                    .usage()
                    .at(FuelChargeSite::Operation(operation_id(2)))
                    .unwrap()
                    .units(),
                1
            );
            assert_eq!(
                measured
                    .usage()
                    .at(FuelChargeSite::Operation(operation_id(3)))
                    .unwrap()
                    .units(),
                1
            );
        }
    }
}

#[test]
fn case_membership_does_not_consume_the_returned_case_or_payload() {
    let mut module = observed_constructor(2, 1, StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: place_id(2),
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: semantic_vocabulary::StructuralPlaceKind::Result,
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: place_id(1),
        returned_claims: vec![],
        trivial_affine_discards: vec![],
    };
    let result = interpret_terminal_artifact_measured(
        &encode_module(&module).unwrap(),
        &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .unwrap();
    let TerminalExecutionResult::ScalarCase(result) = result.value() else {
        panic!("observation must preserve the structural return")
    };
    assert_eq!(result.value.result_case, structural_case_id(2));
    assert_eq!(
        result.value.fields[0].1,
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(37),
        }
    );
}

#[test]
fn case_membership_requires_its_constructor_before_the_observation() {
    for multiplicity in [
        StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine,
    ] {
        let mut module = observed_constructor(1, 1, multiplicity);
        module.machines[0].blocks[0].operations.swap(1, 2);
        assert!(matches!(
            terminal_verifier::validate_module(&module),
            Err(ModuleError::InvalidStructuralCaseObservation { .. }),
        ));
    }
}

#[test]
fn case_membership_rejects_a_constructor_that_does_not_dominate_the_join() {
    let mut module = observed_constructor(1, 1, StructuralMultiplicity::Unrestricted);
    let machine = &mut module.machines[0];
    let mut joined = machine.blocks.remove(0);
    let mut prefix = joined.operations.drain(..2).collect::<Vec<_>>();
    let constructor = prefix.pop().unwrap();
    prefix[0].result.scalar_mut().unwrap().scalar_type = ScalarType::Boolean;
    prefix[0].kind = OperationKind::BooleanConstant { value: true };
    joined.id = block_id(4);
    joined.terminator = Terminator::Return {
        edge: edge_id(5),
        value: value_id(2),
        cleanup_actions: vec![],
    };
    let branch = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: vec![],
        erased_arguments: Vec::new(),
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
    };
    let jump = |edge| Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(4),
        arguments: vec![],
        erased_arguments: Vec::new(),
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            id: block_id(1),
            parameters: vec![],
            structural_parameters: vec![],
            operations: prefix,
            terminator: Terminator::Conditional {
                condition: value_id(1),
                when_true: branch(1, 2),
                when_false: branch(2, 3),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            id: block_id(2),
            parameters: vec![],
            structural_parameters: vec![],
            operations: vec![constructor],
            terminator: jump(3),
        },
        Block {
            erased_scalar_formals: Vec::new(),
            id: block_id(3),
            parameters: vec![],
            structural_parameters: vec![],
            operations: vec![],
            terminator: jump(4),
        },
        joined,
    ];
    assert!(matches!(
        terminal_verifier::validate_module(&module),
        Err(ModuleError::InvalidStructuralCaseObservation { .. }),
    ));
}

#[test]
fn case_membership_cannot_read_an_owner_discarded_on_an_earlier_edge() {
    let mut module = observed_constructor(1, 1, StructuralMultiplicity::Affine);
    let machine = &mut module.machines[0];
    let mut continuation = machine.blocks.remove(0);
    let prefix = continuation.operations.drain(..2).collect::<Vec<_>>();
    continuation.id = block_id(2);
    continuation.terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(2),
        cleanup_actions: vec![],
    };
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            id: block_id(1),
            parameters: vec![],
            structural_parameters: vec![],
            operations: prefix,
            terminator: Terminator::Jump {
                edge: edge_id(1),
                target: block_id(2),
                arguments: vec![],
                erased_arguments: Vec::new(),
                structural_arguments: vec![],
                trivial_affine_discards: vec![place_id(1)],
                residual_affine_discards: vec![],
            },
        },
        continuation,
    ];
    assert!(matches!(
        terminal_verifier::validate_module(&module),
        Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { operation, place })
            if operation == operation_id(3) && place == place_id(1),
    ));
}

#[test]
fn case_membership_suspension_charges_each_observation_once() {
    let module = observed_constructor(2, 1, StructuralMultiplicity::Affine);
    let mut execution = TerminalExecution::start_artifact(
        &encode_module(&module).unwrap(),
        &encode_proof_section(&module, &ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(2);
    for operation in [3, 4] {
        let TerminalExecutionStatus::SponsorExhausted(exhaustion) = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap()
        else {
            panic!("observation must wait for fuel")
        };
        assert_eq!(
            exhaustion.site,
            FuelChargeSite::Operation(operation_id(operation))
        );
        assert_eq!(execution.live_affine_frontier().count(), 1);
        meter.replenish(1).unwrap();
    }
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(false)
        )),
    );
    assert_eq!(meter.usage().total_units(), 5);
    assert_eq!(execution.live_affine_frontier().count(), 0);
}
