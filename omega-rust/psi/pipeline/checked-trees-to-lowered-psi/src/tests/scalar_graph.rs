//! Scalar-graph module assembly regressions.

use super::*;

#[test]
fn scalar_graph_replays_parameter_qualification_contracts_from_source() {
    let original = checked_source(include_str!(
        "../../../../../../tests/omega/pass/expressions/qualified_call_result_argument/main.omg"
    ));
    lower_machine(&original, "choose").expect("qualified argument composition");
    let relay = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay");
    let state = &original.machine_states(relay)[0];
    let contract = state.contracts.start();
    let fact = original.signature_contracts.get(contract).facts.start();
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed.typed.signature_contracts.get_mut(contract).kind =
                    checked_trees::signature::SignatureContractKind::Ensures
            }
            1 => {
                let checked_trees::domain::ProofFact::Membership(membership) =
                    changed.typed.proof_facts.get_mut(fact)
                else {
                    panic!("membership")
                };
                membership.domain_symbol = SymbolHandle::invalid();
            }
            2 => {
                let checked_trees::domain::ProofFact::Membership(membership) =
                    changed.typed.proof_facts.get_mut(fact)
                else {
                    panic!("membership")
                };
                membership.value = checked_trees::expression::ExpressionHandle::invalid();
            }
            _ => {
                *changed.typed.proof_facts.get_mut(fact) =
                    checked_trees::domain::ProofFact::Expression(
                        checked_trees::expression::ExpressionHandle::invalid(),
                    )
            }
        }
        let error = lower_machine(&changed, "choose").expect_err("source contract changed");
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(
                    "scalar state contract is not carried by its qualified signature"
                )
            ),
            "wrong rejection: {error:?}"
        );
    }
}

#[test]
fn scalar_completion_after_array_calls_replays_its_expression_and_statement_order() {
    let checked = checked_source(include_str!(
        "../../../../../../tests/omega/pass/collections/owned_array_scalar_comparisons/main.omg"
    ));
    for entry in ["scalar_comparison", "array_comparison"] {
        let artifact = produce_terminal_artifact(&checked, entry)
            .expect("comparison completes the ordered array/call body in Terminal");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        let profile = proof_admission::AdmissionProfile::default();
        terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
        for value in [65535, 42] {
            let argument = terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    16,
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
            };
            let result = terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[argument],
            )
            .unwrap();
            let expected = terminal_interpreter::TerminalScalarValue::Boolean(value == 65535);
            match (entry, result) {
                (
                    "scalar_comparison",
                    terminal_interpreter::TerminalExecutionResult::Scalar(result),
                ) => assert_eq!(result, expected),
                (
                    "array_comparison",
                    terminal_interpreter::TerminalExecutionResult::ScalarArray(result),
                ) => assert_eq!(
                    result.value.elements,
                    [
                        expected,
                        terminal_interpreter::TerminalScalarValue::Boolean(true)
                    ]
                ),
                (_, result) => panic!("unexpected {entry} result: {result:?}"),
            }
        }
    }
    let selected = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "scalar_comparison")
        .unwrap()
        .symbol;
    for corruption in 0..4 {
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == selected)
            .expect("ordered scalar completion owner");
        let result = plan.scalar_result.unwrap();
        match corruption {
            0 => plan.operations.retain(|operation| {
                !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                )
            }),
            1 => {
                let value = plan
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                            result: candidate,
                            value,
                        } if *candidate == result => Some(value),
                        _ => None,
                    })
                    .unwrap();
                *value = checked_trees::CheckedCallScalarArgument::Pure(
                    CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(
                        true,
                    ))),
                );
            }
            2 => plan.scalar_result.as_mut().unwrap().statement_index = 0,
            3 => {
                let final_expression = plan
                    .operations
                    .iter()
                    .position(|operation| {
                        matches!(operation, CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result: candidate, ..
                    } if *candidate == result)
                    })
                    .unwrap();
                plan.operations.swap(0, final_expression);
            }
            _ => unreachable!(),
        }
        assert!(
            produce_terminal_artifact(&changed, "scalar_comparison").is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn scalar_machine_builder_uses_a_disjoint_module_identity_namespace() {
    let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
    let lowered = build_scalar_graph_module(
        &[LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean.into()],
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: LoweredDirectExpression::Boolean {
                    expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
                },
            },
        }],
        ScalarType::Boolean.into(),
        &terminal_psi::ScalarQualificationCatalog::default(),
        PreparedScalarContract::Empty,
        Vec::new(),
        LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        },
        LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        },
        machine_id(2),
        identity_base,
        &[],
        &[],
        None,
    )
    .expect("a nonentry machine should lower in its disjoint identity range");

    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("the isolated builder emits one machine")
    };
    assert_eq!(machine.id, machine_id(2));
    assert_eq!(machine.contract.id, contract_id(2));
    assert_eq!(machine.entry, block_id(identity_base + 1));
    assert_eq!(machine.parameters[0].id, value_id(identity_base + 1));
    assert_eq!(
        machine
            .result
            .scalar()
            .expect("the scalar fixture has a result")
            .id,
        value_id(identity_base + 2)
    );
    let Terminator::Return { edge, value, .. } = machine.blocks[0].terminator else {
        panic!("the fixture should retain its scalar return")
    };
    assert_eq!(edge, edge_id(identity_base + 1));
    assert_eq!(value, value_id(identity_base + 1));
}

#[test]
fn primitive_scalar_source_jump_emits_empty_affine_cleanup() {
    let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
    let parameter_expression = || LoweredDirectExpression::Boolean {
        expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
    };
    let lowered = build_scalar_graph_module(
        &[
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean.into()],
                bindings: Vec::new(),
                structural_effects: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    structural_arguments: Vec::new(),
                    target: 1,
                    arguments: vec![parameter_expression()],
                },
            },
            LoweredScalarBranchState {
                parameter_types: vec![ScalarType::Boolean.into()],
                bindings: Vec::new(),
                structural_effects: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Return {
                    expression: parameter_expression(),
                },
            },
        ],
        ScalarType::Boolean.into(),
        &terminal_psi::ScalarQualificationCatalog::default(),
        PreparedScalarContract::Empty,
        Vec::new(),
        LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        },
        LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        },
        machine_id(2),
        identity_base,
        &[],
        &[],
        None,
    )
    .expect("primitive scalar jump should lower");

    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &lowered.semantic_module.machines[0].blocks[0].terminator
    else {
        panic!("first scalar block should jump")
    };
    assert!(trivial_affine_discards.is_empty());
}

#[test]
fn primitive_scalar_source_conditional_emits_empty_affine_cleanup() {
    let identity_base = TERMINAL_MACHINE_IDENTITY_STRIDE;
    let parameter_expression = || LoweredDirectExpression::Boolean {
        expression: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
    };
    let states = [
        LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean.into()],
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Conditional {
                condition: LoweredBooleanReturnExpression::Parameter { position: 0 },
                when_true_target: 1,
                when_true_arguments: vec![parameter_expression()],
                when_false_target: 2,
                when_false_arguments: vec![parameter_expression()],
            },
        },
        LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean.into()],
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: parameter_expression(),
            },
        },
        LoweredScalarBranchState {
            parameter_types: vec![ScalarType::Boolean.into()],
            bindings: Vec::new(),
            structural_effects: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: parameter_expression(),
            },
        },
    ];
    let lowered = build_scalar_graph_module(
        &states,
        ScalarType::Boolean.into(),
        &terminal_psi::ScalarQualificationCatalog::default(),
        PreparedScalarContract::Empty,
        Vec::new(),
        LoweredContentIdentityReshuffles {
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            reshuffles: Vec::new(),
            source_claims: Vec::new(),
        },
        LoweredContentPartitionCompositions {
            structural_places: Vec::new(),
            compositions: Vec::new(),
        },
        machine_id(2),
        identity_base,
        &[],
        &[],
        None,
    )
    .expect("primitive scalar conditional should lower");

    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &lowered.semantic_module.machines[0].blocks[0].terminator
    else {
        panic!("first scalar block should branch")
    };
    assert!(when_true.trivial_affine_discards.is_empty());
    assert!(when_false.trivial_affine_discards.is_empty());
}
