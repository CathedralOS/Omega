//! Structural scalar-return regression families.

use super::*;

fn structural_scalar_return_fixture(parameters: &str, result: &str, body: &str) -> CheckedTrees {
    checked_source(&format!(
        "data Root {{}}
         data Acknowledgement {{}}
         machine Root::enter({parameters}) -> {result} {{ {body} }}"
    ))
}

#[test]
fn structural_scalar_return_lowers_value_before_exact_affine_cleanup() {
    let checked = structural_scalar_return_fixture(
        "first: Acknowledgement, second: Acknowledgement",
        "i32",
        "7i32",
    );

    let lowered = lower_machine(&checked, "Root::enter")
        .expect("closed scalar return and exact affine cleanup should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("structural scalar return lowers one attached machine")
    };
    assert_eq!(machine.structural_parameters.len(), 2);
    assert!(machine.parameters.is_empty());
    assert!(matches!(machine.result, TerminalMachineResult::Scalar(_)));
    let [block] = machine.blocks.as_slice() else {
        panic!("closed structural scalar return lowers one block")
    };
    assert!(matches!(
        &block.terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));
    assert!(matches!(
        block.operations.as_slice(),
        [Operation {
            kind: OperationKind::IntegerConstant { .. },
            ..
        }]
    ));
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("structural scalar return should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("canonical structural scalar return bytes should decode"),
        lowered.semantic_module
    );
}

#[test]
fn structural_scalar_return_fails_closed_on_stale_cleanup() {
    let mut checked = structural_scalar_return_fixture(
        "first: Acknowledgement, second: Acknowledgement",
        "i32",
        "7i32",
    );

    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .cleanup_actions = vec![
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0),
        CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
    ];

    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar return cleanup does not consume its exact frontier"
        ))
    ));
}

#[test]
fn structural_scalar_return_reconstructs_closed_exact_expression_proof() {
    let mut checked = structural_scalar_return_fixture(
        "first: Acknowledgement, second: Acknowledgement",
        "i32",
        "3i32 + 4i32",
    );

    let lowered = lower_machine(&checked, "Root::enter")
        .expect("closed exact expression should lower with reconstructed proof");
    let operations = &lowered.semantic_module.machines[0].blocks[0].operations;
    assert!(matches!(
        operations.as_slice(),
        [
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::ExactIntegerAdd { .. },
                ..
            }
        ]
    ));
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the reconstructed exact-operation proof should verify canonically");
    let module_bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("closed structural expression module should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&module_bytes)
            .expect("closed structural expression module should decode canonically"),
        lowered.semantic_module
    );
    assert!(matches!(
        &lowered.semantic_module.machines[0].blocks[0].terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));

    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I32,
        };
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar return is outside its checked value/control slice"
        ))
    ));
}

#[test]
fn structural_scalar_return_materializes_branch_free_local_prefix_before_cleanup() {
    let mut checked = structural_scalar_return_fixture(
        "first: Acknowledgement, second: Acknowledgement",
        "i32",
        "let sum: i32 = 3i32 + 4i32; sum",
    );

    let lowered = lower_machine(&checked, "Root::enter")
        .expect("checked local prefix should lower before exact affine cleanup");
    let block = &lowered.semantic_module.machines[0].blocks[0];
    assert!(matches!(
        block.operations.as_slice(),
        [
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            Operation {
                kind: OperationKind::ExactIntegerAdd { .. },
                ..
            }
        ]
    ));
    assert!(matches!(
        &block.terminator,
        Terminator::Return {
            value,
            cleanup_actions,
            ..
        } if *value == value_id(3)
            && cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
    ));
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("local-prefix cleanup module should verify");

    checked.facts.values.scalar_expressions.expressions[0].expression =
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::I32,
        };
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar binding is not one branch-free local expression"
        ))
    ));
}

#[test]
fn structural_scalar_return_supports_repeated_carried_short_circuit_local_continuations() {
    let mut checked = structural_scalar_return_fixture(
        "left: Acknowledgement, right: Acknowledgement",
        "bool",
        "let first: bool = true;
         let second: bool = first && false;
         let third: bool = !second;
         let fourth: bool = third || false;
         let fifth: bool = !fourth;
         let sixth: bool = fifth && true;
         let seventh: bool = !sixth;
         seventh",
    );

    let lowered = lower_machine(&checked, "Root::enter")
        .expect("repeated short-circuit locals should compose through carried continuations");
    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 16);
    let second_stage = machine
        .blocks
        .iter()
        .find(|block| block.id == block_id(6))
        .expect("the first short-circuit result enters the second decision stage");
    let third_stage = machine
        .blocks
        .iter()
        .find(|block| block.id == block_id(11))
        .expect("the second short-circuit result enters the third decision stage");
    let continuation = machine
        .blocks
        .iter()
        .find(|block| block.id == block_id(16))
        .expect("the final short-circuit result enters the return continuation");
    assert!(matches!(
        machine.blocks[0].operations.first(),
        Some(Operation {
            kind: OperationKind::BooleanConstant { value: true },
            ..
        })
    ));
    assert!(matches!(
        second_stage.parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        second_stage.operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(matches!(
        third_stage.parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        third_stage.operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(matches!(
        continuation.parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        continuation.operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(matches!(
        &continuation.terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));
    assert!(
        machine.blocks[..15]
            .iter()
            .all(|block| match &block.terminator {
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } =>
                    when_true.trivial_affine_discards.is_empty()
                        && when_false.trivial_affine_discards.is_empty(),
                Terminator::Jump {
                    target,
                    trivial_affine_discards,
                    ..
                } => {
                    matches!(*target, target if target == block_id(6)
                            || target == block_id(11)
                            || target == block_id(16))
                        && trivial_affine_discards.is_empty()
                }
                _ => false,
            })
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("short-circuit local convergence should preserve the structural frontier");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("short-circuit local cleanup should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("short-circuit local cleanup should decode canonically"),
        lowered.semantic_module
    );

    checked.facts.values.scalar_expressions.expressions[7].expression =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::Local { position: 6 }),
            right: Box::new(CheckedBooleanExpression::Constant(false)),
        }));
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("repeated local decisions should feed a final short-circuit return");
    let machine = &lowered.semantic_module.machines[0];
    assert_eq!(machine.blocks.len(), 20);
    let final_decision = &machine.blocks[15..];
    assert!(matches!(
        final_decision[0].parameters.as_slice(),
        [ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        }]
    ));
    assert!(matches!(
        final_decision[0].operations.as_slice(),
        [Operation {
            kind: OperationKind::BooleanNot { .. },
            ..
        }]
    ));
    assert!(final_decision.iter().all(|block| match &block.terminator {
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            when_true.trivial_affine_discards.is_empty()
                && when_false.trivial_affine_discards.is_empty()
        }
        Terminator::Return {
            cleanup_actions, ..
        } =>
            cleanup_actions
                == &[
                    TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                    TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                ],
        _ => false,
    }));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("final short-circuit cleanup should verify after repeated local convergence");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("composed final short-circuit cleanup should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("composed final short-circuit cleanup should decode canonically"),
        lowered.semantic_module
    );

    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .bindings[5]
        .primitive_type = PrimitiveType::I32;
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar short-circuit binding has a non-Boolean carrier"
        ))
    ));
}

#[test]
fn structural_scalar_return_maps_interleaved_scalar_parameters_before_cleanup() {
    let mut checked = structural_scalar_return_fixture(
        "first: Acknowledgement, number: i32, second: Acknowledgement, flag: bool",
        "bool",
        "!flag",
    );

    let lowered = lower_machine(&checked, "Root::enter")
        .expect("exact mixed parameter map should lower before affine cleanup");
    let machine = &lowered.semantic_module.machines[0];
    assert!(matches!(
        machine.parameters.as_slice(),
        [
            ValueDeclaration {
                id,
                scalar_type: ScalarType::Integer(_),
            },
            ValueDeclaration {
                id: bool_id,
                scalar_type: ScalarType::Boolean,
            }
        ] if *id == value_id(1) && *bool_id == value_id(2)
    ));
    assert_eq!(machine.structural_parameters.len(), 2);
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [Operation {
            result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                id,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanNot { operand },
            ..
        }] if *id == value_id(3) && *operand == value_id(2)
    ));
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Return {
            value,
            cleanup_actions,
            ..
        } if *value == value_id(3)
            && cleanup_actions == &[
                TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
            ]
    ));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("mixed scalar/structural parameter module should verify");

    checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .scalar_parameters[0]
        .source_position = 0;
    assert!(matches!(
        lower_machine(&checked, "Root::enter"),
        Err(LoweringError::Unsupported(
            "structural scalar return parameter maps overlap or repeat a source position"
        ))
    ));
}

#[test]
fn structural_scalar_return_emits_boolean_paths_before_cleanup() {
    let checked = structural_scalar_return_fixture(
        "first: Acknowledgement, second: Acknowledgement",
        "bool",
        "!(true == false)",
    );

    let lowered = lower_machine(&checked, "Root::enter")
        .expect("closed branch-free Boolean should lower before structural cleanup");
    let machine = &lowered.semantic_module.machines[0];
    assert!(matches!(
        machine.result,
        TerminalMachineResult::Scalar(ValueDeclaration {
            scalar_type: ScalarType::Boolean,
            ..
        })
    ));
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [
            Operation {
                kind: OperationKind::BooleanConstant { value: true },
                ..
            },
            Operation {
                kind: OperationKind::BooleanConstant { value: false },
                ..
            },
            Operation {
                kind: OperationKind::BooleanEqual { .. },
                ..
            },
            Operation {
                kind: OperationKind::BooleanNot { .. },
                ..
            }
        ]
    ));
    assert!(matches!(
        &machine.blocks[0].terminator,
        Terminator::Return {
            cleanup_actions,
            ..
        } if cleanup_actions == &[
            TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
            TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
        ]
    ));
    assert!(lowered.proof_bundle.evidence.is_empty());
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("closed Boolean return and cleanup should verify");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("closed Boolean cleanup module should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("closed Boolean cleanup module should decode canonically"),
        lowered.semantic_module
    );

    let checked = structural_scalar_return_fixture(
        "first: Acknowledgement, second: Acknowledgement",
        "bool",
        "let selected: bool = true; selected && false",
    );
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("short-circuit Boolean leaves should each perform exact affine cleanup");
    let blocks = &lowered.semantic_module.machines[0].blocks;
    assert_eq!(blocks.len(), 5);
    assert!(matches!(
        blocks[0].operations.first(),
        Some(Operation {
            kind: OperationKind::BooleanConstant { value: true },
            ..
        })
    ));
    let mut return_count = 0;
    for block in blocks {
        match &block.terminator {
            Terminator::Return {
                cleanup_actions, ..
            } => {
                return_count += 1;
                assert_eq!(
                    cleanup_actions,
                    &[
                        TerminalAffineCleanupAction::DiscardRoot(place_id(2)),
                        TerminalAffineCleanupAction::DiscardRoot(place_id(1)),
                    ]
                );
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                assert!(when_true.trivial_affine_discards.is_empty());
                assert!(when_false.trivial_affine_discards.is_empty());
            }
            _ => panic!("short-circuit return emits only decisions and scalar leaves"),
        }
    }
    assert_eq!(return_count, 3);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("short-circuit cleanup frontiers should verify on every path");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("short-circuit structural cleanup should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("short-circuit structural cleanup should decode canonically"),
        lowered.semantic_module
    );
}
