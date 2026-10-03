use super::{
    AcceptTerminalEffects, AdmissionProfile, OperationKind, ScalarType, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalArtifactInterpretError, TerminalExecutionResult,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralValue, Terminator, decode_module, decode_proof_bundle,
    derive_fixed_entry_fuel, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured, validate_fixed_entry_fuel,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::TerminalStructuralInputs;

const MIXED_CONTEXTUAL_SCALAR_BINDINGS_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(owner: &mut Token)
    requires owner.ready
    {}

    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(first: Token, plain: Plain, second: Token) -> bool
    requires first.ready, plain.observed, second.ready
    {
        let ready: bool = true;
        let inverted: bool = !ready;
        !inverted
    }
"#;

const MIXED_CONTEXTUAL_SCALAR_INPUTS_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(owner: &mut Token)
    requires owner.ready
    {}

    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(
        first: Token,
        left: bool,
        plain: Plain,
        right: bool,
        second: Token
    ) -> bool
    requires first.ready, plain.observed, second.ready
    {
        let same: bool = left == right;
        let inverted: bool = !same;
        !inverted
    }
"#;

const MIXED_NOMINAL_SHORT_CIRCUIT_SCALAR_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(owner: &mut Token) { Helper::touch(); }
    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(
        token: Token,
        left: bool,
        plain: Plain,
        right: bool
    ) -> bool
    {
        let inverted: bool = !right;
        left && inverted
    }
"#;

const MIXED_NOMINAL_NESTED_SHORT_CIRCUIT_SCALAR_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(owner: &mut Token) { Helper::touch(); }
    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(
        token: Token,
        left: bool,
        plain: Plain,
        right: bool
    ) -> bool
    {
        let staged: bool = left && (right || !left);
        let continued: bool = staged || (left && right);
        continued
    }
"#;

const MIXED_NOMINAL_SHARED_BOOLEAN_CONVERGENCE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { ready: bool; }
    machine Token::drop(owner: &mut Token) { Helper::touch(); }
    data Root {}
    machine Root::measure(token: Token, left: bool) -> bool {
        let staged: bool = token.ready && !left;
        staged
    }
"#;

#[test]
fn mixed_contextual_scalar_return_materializes_branch_free_bindings_before_cleanup() {
    let checked = crate::front_end::checked_program(MIXED_CONTEXTUAL_SCALAR_BINDINGS_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::measure"),
    )
    .expect("mixed contextual scalar bindings lower");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed contextual scalar bindings entry");
    let [first, plain, second] = entry.structural_parameters.as_slice() else {
        panic!("binding entry retains its complete structural signature")
    };
    let [ready, inverted, result] = entry.blocks[0].operations.as_slice() else {
        panic!("two bindings and the return expression materialize in source order")
    };
    assert!(matches!(ready.kind, OperationKind::BooleanConstant { .. }));
    assert!(matches!(inverted.kind, OperationKind::BooleanNot { .. }));
    assert!(matches!(result.kind, OperationKind::BooleanNot { .. }));
    assert!(
        entry.blocks[0]
            .operations
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
    );
    let Terminator::Return {
        value,
        cleanup_actions,
        ..
    } = &entry.blocks[0].terminator
    else {
        panic!("binding entry returns its scalar result")
    };
    assert_eq!(result.result.scalar().expect("result value").id, *value);
    assert!(matches!(
        cleanup_actions.as_slice(),
        [
            TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
            TerminalAffineCleanupAction::DiscardRoot(plain_cleanup),
            TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
        ] if second_cleanup.place == second.place
            && *plain_cleanup == plain.place
            && first_cleanup.place == first.place
            && second_cleanup.cleanup_machine == first_cleanup.cleanup_machine
    ));
    let obligations: Vec<_> = cleanup_actions
        .iter()
        .filter_map(|action| match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => Some(cleanup),
            _ => None,
        })
        .flat_map(|cleanup| cleanup.requirement_obligations.iter().copied())
        .collect();
    assert_eq!(obligations.len(), 2);
    assert_ne!(obligations[0], obligations[1]);
    for obligation in obligations {
        assert!(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .any(|evidence| { evidence.obligation == obligation })
        );
        assert!(
            entry.blocks[0]
                .operations
                .iter()
                .all(|operation| { operation.id.get() != obligation.get() })
        );
    }
    assert_eq!(entry.contract.requires.len(), 3);
    assert_eq!(
        lowered.proof_bundle.evidence.len(),
        2,
        "each cleanup invocation retains its own proof obligation",
    );

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed contextual scalar bindings verify");
    let semantics = encode_module(&lowered.semantic_module).expect("binding module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("binding proof encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let structural_arguments = [first, plain, second].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    let mut handler = AcceptTerminalEffects;
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
        &mut handler,
    )
    .expect("mixed contextual scalar bindings interpret from canonical artifacts");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
    );
    assert_eq!(measured.usage().total_units(), 6);
    assert!(measured.effects().is_empty());
}

#[test]
fn mixed_contextual_scalar_return_preserves_interleaved_primitive_inputs() {
    let checked = crate::front_end::checked_program(MIXED_CONTEXTUAL_SCALAR_INPUTS_SOURCE);
    let checked_plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .find(|plan| !plan.scalar_parameters.is_empty())
        .expect("checked mixed signature is retained");
    assert_eq!(
        checked_plan
            .structural_parameters
            .iter()
            .map(|parameter| parameter.position)
            .collect::<Vec<_>>(),
        [0, 2, 4]
    );
    assert_eq!(
        checked_plan
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::measure"),
    )
    .expect("mixed contextual scalar inputs lower");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed contextual scalar inputs entry");
    let [left, right] = entry.parameters.as_slice() else {
        panic!("primitive inputs become a dense scalar ABI")
    };
    assert_eq!([left.id.get(), right.id.get()], [1, 2]);
    assert_eq!(left.scalar_type, ScalarType::Boolean);
    assert_eq!(right.scalar_type, ScalarType::Boolean);
    let [first, plain, second] = entry.structural_parameters.as_slice() else {
        panic!("interleaved signature retains all structural roots")
    };
    assert_eq!([first.position, plain.position, second.position], [0, 1, 2]);

    let [same, inverted, result] = entry.blocks[0].operations.as_slice() else {
        panic!("input-dependent bindings and return materialize in source order")
    };
    assert!(matches!(
        same.kind,
        OperationKind::BooleanEqual {
            left: operand_left,
            right: operand_right,
        } if operand_left == left.id && operand_right == right.id
    ));
    let same_value = same.result.scalar().expect("equality result").id;
    assert_eq!(same_value.get(), 3, "locals begin after both ABI inputs");
    assert!(matches!(
        inverted.kind,
        OperationKind::BooleanNot { operand } if operand == same_value
    ));
    let inverted_value = inverted.result.scalar().expect("inversion result").id;
    assert_eq!(inverted_value.get(), 4);
    assert!(matches!(
        result.kind,
        OperationKind::BooleanNot { operand } if operand == inverted_value
    ));
    assert_eq!(result.result.scalar().expect("return result").id.get(), 5);
    let Terminator::Return {
        value,
        cleanup_actions,
        ..
    } = &entry.blocks[0].terminator
    else {
        panic!("mixed contextual scalar input entry returns its scalar result")
    };
    assert_eq!(result.result.scalar().expect("return result").id, *value);
    assert!(matches!(
        cleanup_actions.as_slice(),
        [
            TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
            TerminalAffineCleanupAction::DiscardRoot(plain_cleanup),
            TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
        ] if second_cleanup.place == second.place
            && *plain_cleanup == plain.place
            && first_cleanup.place == first.place
            && second_cleanup.cleanup_machine == first_cleanup.cleanup_machine
    ));
    assert_eq!(entry.contract.requires.len(), 3);
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed contextual scalar inputs verify");
    let semantics = encode_module(&lowered.semantic_module).expect("input module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("input proof encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let scalar_arguments = [
        TerminalScalarValue::Boolean(true),
        TerminalScalarValue::Boolean(false),
    ];
    let structural_arguments = [first, plain, second].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    let mut handler = AcceptTerminalEffects;
    let measured = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &scalar_arguments,
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
        &mut handler,
    )
    .expect("mixed contextual scalar inputs interpret from canonical artifacts");
    assert_eq!(
        measured.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
    );
    assert_eq!(measured.usage().total_units(), 6);
    assert!(measured.effects().is_empty());
}

#[test]
fn mixed_nominal_scalar_return_cleans_every_short_circuit_leaf() {
    let checked = crate::front_end::checked_program(MIXED_NOMINAL_SHORT_CIRCUIT_SCALAR_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::measure"),
    )
    .expect("mixed nominal short-circuit scalar return lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed nominal short-circuit entry");
    let [left, right] = entry.parameters.as_slice() else {
        panic!("interleaved primitive inputs become one dense scalar namespace")
    };
    assert_eq!([left.id.get(), right.id.get()], [1, 2]);
    assert_eq!(left.scalar_type, ScalarType::Boolean);
    assert_eq!(right.scalar_type, ScalarType::Boolean);
    let [token, plain] = entry.structural_parameters.as_slice() else {
        panic!("mixed nominal short-circuit entry retains both structural roots")
    };
    assert_eq!([token.position, plain.position], [0, 1]);
    assert_eq!(entry.blocks.len(), 5);
    assert!(matches!(
        entry.blocks[0].operations.first(),
        Some(terminal_psi::Operation {
            kind: OperationKind::BooleanNot { operand },
            ..
        }) if *operand == right.id
    ));

    let mut return_edges = Vec::new();
    let mut return_count = 0;
    let mut conditional_count = 0;
    let mut expected_cleanup = None;
    for block in &entry.blocks {
        match &block.terminator {
            Terminator::Return {
                edge,
                cleanup_actions,
                ..
            } => {
                return_count += 1;
                return_edges.push(*edge);
                assert!(matches!(
                    cleanup_actions.as_slice(),
                    [
                        TerminalAffineCleanupAction::DiscardRoot(plain_cleanup),
                        TerminalAffineCleanupAction::InvokeNominal(token_cleanup),
                    ] if *plain_cleanup == plain.place
                        && token_cleanup.place == token.place
                ));
                match &expected_cleanup {
                    Some(expected) => assert_eq!(cleanup_actions, expected),
                    None => expected_cleanup = Some(cleanup_actions.clone()),
                }
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                conditional_count += 1;
                assert!(when_true.trivial_affine_discards.is_empty());
                assert!(when_false.trivial_affine_discards.is_empty());
            }
            _ => panic!("one final short-circuit return emits only decisions and value leaves"),
        }
    }
    assert_eq!(conditional_count, 2);
    assert_eq!(return_count, 3);
    return_edges.sort_unstable();
    return_edges.dedup();
    assert_eq!(
        return_edges.len(),
        3,
        "each value leaf owns its return edge"
    );
    let [
        TerminalAffineCleanupAction::DiscardRoot(_),
        TerminalAffineCleanupAction::InvokeNominal(token_cleanup),
    ] = expected_cleanup
        .as_deref()
        .expect("every return leaf retains cleanup")
    else {
        panic!("mixed cleanup has one no-code action and one nominal action")
    };
    assert!(token_cleanup.requirement_obligations.is_empty());
    let cleanup_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == token_cleanup.cleanup_machine)
        .expect("nominal cleanup target remains in the terminal closure");
    let [receiver] = cleanup_target.structural_parameters.as_slice() else {
        panic!("named borrowed cleanup receiver remains in the target signature")
    };
    assert_eq!(token_cleanup.cleanup_receiver, Some(receiver.place));
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.structural_type, token.structural_type);
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert!(matches!(
        cleanup_target.blocks[0].operations.as_slice(),
        [terminal_psi::Operation {
            kind: OperationKind::CallUnit { .. },
            ..
        }]
    ));
    assert!(lowered.proof_bundle.evidence.is_empty());

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed nominal short-circuit cleanup verifies on every leaf");
    let semantics = encode_module(&lowered.semantic_module)
        .expect("mixed nominal short-circuit module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("mixed nominal short-circuit proof encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let structural_arguments = [token, plain].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    for (scalar_arguments, expected, expected_fuel) in [
        (
            [
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(false),
            ],
            false,
            7,
        ),
        (
            [
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
            true,
            8,
        ),
    ] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &scalar_arguments,
            TerminalStructuralInputs {
                arguments: &structural_arguments,
                ..Default::default()
            },
            &mut handler,
        )
        .expect("mixed nominal short-circuit path interprets from canonical artifacts");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_fuel);
        assert!(measured.effects().is_empty());
    }
}

#[test]
fn mixed_nominal_scalar_return_cleans_every_nested_short_circuit_leaf() {
    let checked =
        crate::front_end::checked_program(MIXED_NOMINAL_NESTED_SHORT_CIRCUIT_SCALAR_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::measure"),
    )
    .expect("nested nominal short-circuit scalar return lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("nested nominal short-circuit entry");
    let [token, plain] = entry.structural_parameters.as_slice() else {
        panic!("nested nominal short-circuit entry retains both structural roots")
    };
    let mut conditional_count = 0;
    let mut return_count = 0;
    for block in &entry.blocks {
        match &block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                conditional_count += 1;
                assert!(when_true.trivial_affine_discards.is_empty());
                assert!(when_false.trivial_affine_discards.is_empty());
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                return_count += 1;
                assert!(matches!(
                    cleanup_actions.as_slice(),
                    [
                        TerminalAffineCleanupAction::DiscardRoot(plain_cleanup),
                        TerminalAffineCleanupAction::InvokeNominal(token_cleanup),
                    ] if *plain_cleanup == plain.place && token_cleanup.place == token.place
                ));
            }
            _ => panic!("nested nominal cleanup emits only decisions and return leaves"),
        }
    }
    assert!(
        conditional_count >= 4,
        "nested and repeated short-circuit stages must retain the full decision tree"
    );
    assert_eq!(return_count, conditional_count + 1);

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested nominal short-circuit cleanup verifies on every leaf");
    let semantics = encode_module(&lowered.semantic_module)
        .expect("nested nominal short-circuit module encodes");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("nested nominal short-circuit proof encodes");
    let structural_arguments = [token, plain].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    for (left, right) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(right),
            ],
            TerminalStructuralInputs {
                arguments: &structural_arguments,
                ..Default::default()
            },
            &mut handler,
        )
        .expect("nested nominal short-circuit path interprets");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(left && right))
        );
        assert!(measured.effects().is_empty());
    }
}

#[test]
fn mixed_nominal_boolean_value_converges_before_one_shared_cleanup_return() {
    let checked =
        crate::front_end::checked_program(MIXED_NOMINAL_SHARED_BOOLEAN_CONVERGENCE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::measure"),
    )
    .expect("shared nominal Boolean convergence lowers");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("shared nominal Boolean convergence entry");
    let [token] = entry.structural_parameters.as_slice() else {
        panic!("shared convergence retains its nominal cleanup root")
    };
    let (convergence, control_blocks) = entry
        .blocks
        .split_last()
        .expect("shared convergence has control and one return block");
    let mut jump_targets = Vec::new();
    let mut decision_count = 0;
    for block in control_blocks {
        match &block.terminator {
            Terminator::Conditional { .. } => decision_count += 1,
            Terminator::Jump {
                target,
                arguments,
                trivial_affine_discards,
                ..
            } => {
                assert_eq!(arguments.len(), 1);
                assert!(trivial_affine_discards.is_empty());
                jump_targets.push(*target);
            }
            _ => panic!("shared convergence control contains only decisions and value jumps"),
        }
    }
    assert_eq!(decision_count, 2);
    assert!(entry.blocks.iter().all(|block| {
        block
            .operations
            .iter()
            .all(|operation| !matches!(operation.kind, OperationKind::BooleanEqual { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::BooleanNot { .. }))
    }));
    let token_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == token.structural_type)
        .expect("shared member source type");
    let StructuralTypeShape::Record { fields } = &token_type.shape else {
        panic!("shared member source is a record")
    };
    let ready = fields
        .iter()
        .find(|field| field.identity == "ready")
        .expect("canonical ready field identity");
    assert!(entry.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(operation.kind,
                OperationKind::BooleanStructuralField { source, field, .. }
                    if source == token.place && field == ready.id)
        })
    }));
    assert_eq!(
        jump_targets,
        [convergence.id, convergence.id, convergence.id]
    );
    let [converged] = convergence.parameters.as_slice() else {
        panic!("shared convergence must bind one typed Boolean value")
    };
    assert_eq!(converged.scalar_type, ScalarType::Boolean);
    let Terminator::Return {
        cleanup_actions, ..
    } = &convergence.terminator
    else {
        panic!("shared convergence must own the sole cleanup return")
    };
    assert!(matches!(
        cleanup_actions.as_slice(),
        [TerminalAffineCleanupAction::InvokeNominal(token_cleanup)]
            if token_cleanup.place == token.place
    ));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared nominal Boolean convergence verifies");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("shared multiple-input convergence has an exact maximum path");
    validate_fixed_entry_fuel(&verified, &fixed)
        .expect("shared multiple-input convergence fuel recomputes");
    drop(verified);
    let semantics = encode_module(&lowered.semantic_module)
        .expect("shared nominal Boolean convergence encodes");
    assert_eq!(
        decode_module(&semantics).expect("shared convergence decodes"),
        lowered.semantic_module
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("shared nominal Boolean convergence proof encodes");
    let structural_arguments = [token].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    let mut handler = AcceptTerminalEffects;
    let missing = interpret_terminal_artifact_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
        &mut handler,
    )
    .expect_err("every retained structural field input must be supplied before execution");
    assert!(matches!(
        missing,
        TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::StructuralBooleanFieldMissing { source, field }
        ) if source == token.place && field == ready.id
    ));
    for (left, ready_value) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(left)],
            TerminalStructuralInputs {
                arguments: &structural_arguments,
                boolean_fields: &[TerminalStructuralBooleanFieldValue {
                    argument_index: 0,
                    path: Vec::new(),
                    field: ready.id,
                    value: ready_value,
                }],
                ..Default::default()
            },
            &mut handler,
        )
        .expect("shared nominal Boolean convergence interprets");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(ready_value && !left))
        );
        assert!(measured.effects().is_empty());
    }
}

#[test]
fn named_receiver_scalar_cleanup_preserves_executable_places_through_artifacts() {
    for effectful in [false, true] {
        let reach = if effectful { "reaches Audit" } else { "" };
        let effect = if effectful { "Audit::note();" } else { "" };
        let source = format!(
            "boundary trait Audit {{ machine note() reaches Audit terminates; }}
         data Token {{ ready: bool; }}
         machine Token::drop(token: &mut Token) requires token.ready {reach} {{
             {effect}
             token.ready = false;
         }}
         machine observed(keep: bool, first: Token, second: Token) -> bool
         requires first.ready; second.ready {reach} {{ keep }}",
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("observed"),
        )
        .expect("scalar cleanup retains ordinary named receiver custody");
        let semantics = encode_module(&lowered.semantic_module).expect("named cleanup encodes");
        let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("named cleanup proof encodes");
        let module = decode_module(&semantics).expect("named cleanup decodes");
        let proof = decode_proof_bundle(&proof).expect("named cleanup proof decodes");
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
            .expect("named receiver remains the executable hook argument after independent replay");
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let Terminator::Return {
            cleanup_actions, ..
        } = &entry.blocks[0].terminator
        else {
            panic!("scalar result retains its cleanup stream")
        };
        let [
            TerminalAffineCleanupAction::InvokeNominal(second),
            TerminalAffineCleanupAction::InvokeNominal(first),
        ] = cleanup_actions.as_slice()
        else {
            panic!("two owners retain two ordered cleanup calls")
        };
        assert_eq!(second.place, entry.structural_parameters[1].place);
        assert_eq!(first.place, entry.structural_parameters[0].place);
        assert_eq!(first.cleanup_machine, second.cleanup_machine);
        let hook = module
            .machines
            .iter()
            .find(|machine| machine.id == first.cleanup_machine)
            .unwrap();
        assert_eq!(
            first.cleanup_receiver,
            Some(hook.structural_parameters[0].place)
        );
        assert_eq!(second.cleanup_receiver, first.cleanup_receiver);
        assert_eq!(!entry.declared_service_reach.is_empty(), effectful);
        let mut wrong_receiver = module.clone();
        let altered_entry = wrong_receiver
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut altered_entry.blocks[0].terminator
        else {
            unreachable!()
        };
        let TerminalAffineCleanupAction::InvokeNominal(cleanup) = &mut cleanup_actions[0] else {
            unreachable!()
        };
        cleanup.cleanup_receiver = Some(first.place);
        assert!(
            terminal_verifier::validate_module_representation(&wrong_receiver).is_err(),
            "cleanup must lend to the hook's declared receiver, not another owner's place"
        );
        let mut shared_receiver = module.clone();
        shared_receiver
            .machines
            .iter_mut()
            .find(|machine| machine.id == hook.id)
            .unwrap()
            .structural_parameters[0]
            .access = terminal_psi::StructuralAccess::SharedBorrow;
        assert!(
            terminal_verifier::validate_module_representation(&shared_receiver).is_err(),
            "a shared parameter cannot receive an exclusive cleanup loan"
        );
        if effectful {
            assert!(
                module
                    .boundary_machines
                    .iter()
                    .any(|boundary| boundary.unconditional_termination)
            );
        }
    }
}
