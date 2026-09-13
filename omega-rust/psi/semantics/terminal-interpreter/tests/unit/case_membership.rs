use super::*;

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
                source: place_id(1),
                case: structural_case_id(observed_case),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(4),
            result: OperationResult::Scalar(boolean(3)),
            kind: OperationKind::StructuralCaseMembership {
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
                    &encode_proof_bundle(&ProofBundle::default()).unwrap(),
                    &AdmissionProfile::default(),
                    &[],
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
        &encode_proof_bundle(&ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
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
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
    };
    let jump = |edge| Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(4),
        arguments: vec![],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    };
    machine.blocks = vec![
        Block {
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
            id: block_id(2),
            parameters: vec![],
            structural_parameters: vec![],
            operations: vec![constructor],
            terminator: jump(3),
        },
        Block {
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
            id: block_id(1),
            parameters: vec![],
            structural_parameters: vec![],
            operations: prefix,
            terminator: Terminator::Jump {
                edge: edge_id(1),
                target: block_id(2),
                arguments: vec![],
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
        &encode_proof_bundle(&ProofBundle::default()).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(2);
    for operation in [3, 4] {
        let TerminalExecutionStatus::SponsorExhausted(exhaustion) =
            execution.resume(&mut meter).unwrap()
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
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(false)
        )),
    );
    assert_eq!(meter.usage().total_units(), 5);
    assert_eq!(execution.live_affine_frontier().count(), 0);
}
