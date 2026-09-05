use super::*;

const MIXED_NOMINAL_REUSED_SHORT_CIRCUIT_SCALAR_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(
        token: Token,
        left: bool,
        plain: Plain,
        right: bool
    ) -> bool
    {
        let staged: bool = left && right;
        let reused: bool = staged == staged;
        let repeated: bool = reused && left;
        repeated
    }
"#;

const MIXED_CONTEXTUAL_SHORT_CIRCUIT_SCALAR_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    { Helper::touch(); }
    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(
        token: Token,
        left: bool,
        plain: Plain,
        right: bool
    ) -> bool
    requires token.ready, plain.observed
    {
        let inverted: bool = !right;
        let staged: bool = left && inverted;
        let completed: bool = !staged;
        let restored: bool = !completed;
        let inverted_again: bool = !restored;
        inverted_again
    }
"#;

const CONTEXTUAL_SCALAR_EXACT_RESULT_SOURCE: &str = r#"
    data Token { ready: bool; armed: bool; }
    machine Token::drop(&mut self)
    requires self.ready, self.armed
    {}

    data Root {}
    machine Root::measure(first: Token, second: Token) -> u64
    requires first.ready, first.armed, second.ready, second.armed
    { 3u64 + 4u64 }
"#;

#[test]
fn mixed_nominal_scalar_return_source_distributes_reused_short_circuit_value() {
    let tokens = Lexer::new(MIXED_NOMINAL_REUSED_SHORT_CIRCUIT_SCALAR_SOURCE)
        .tokenize()
        .expect("tokenize reused nominal short-circuit scalar return");
    let syntax =
        parse_syntax_trees(&tokens).expect("parse reused nominal short-circuit scalar return");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve reused nominal short-circuit scalar return");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type reused nominal short-circuit scalar return");
    let checked =
        lower_typed_trees(typed).expect("check reused nominal short-circuit scalar return");
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::measure")
        .expect("pure reused short-circuit value source-distributes through nominal cleanup");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("reused nominal short-circuit entry");
    let [token, plain] = entry.structural_parameters.as_slice() else {
        panic!("reused nominal short-circuit entry retains both structural roots")
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
            _ => panic!("source-distributed reuse emits only decisions and cleanup leaves"),
        }
    }
    assert!(
        conditional_count > 2,
        "the later short-circuit stage extends the decision tree"
    );
    assert!(
        return_count > 3,
        "every composed value leaf retains cleanup"
    );
    assert_eq!(
        entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::BooleanEqual { .. }))
            .count(),
        3,
        "the branch-free reuse continuation is source-distributed over the three leaves"
    );

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("reused nominal short-circuit cleanup verifies on every leaf");
    let semantics = encode_module(&lowered.semantic_module)
        .expect("reused nominal short-circuit module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("reused nominal short-circuit proof encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let structural_arguments = [token, plain].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    for (left, right) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(right),
            ],
            &structural_arguments,
            &mut handler,
        )
        .expect("reused nominal short-circuit path interprets from canonical artifacts");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(left))
        );
        assert!(measured.effects().is_empty());
    }
}

#[test]
fn mixed_contextual_scalar_return_proves_cleanup_on_every_short_circuit_leaf() {
    let tokens = Lexer::new(MIXED_CONTEXTUAL_SHORT_CIRCUIT_SCALAR_SOURCE)
        .tokenize()
        .expect("tokenize mixed contextual short-circuit scalar return");
    let syntax =
        parse_syntax_trees(&tokens).expect("parse mixed contextual short-circuit scalar return");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve mixed contextual short-circuit scalar return");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type mixed contextual short-circuit scalar return");
    let checked =
        lower_typed_trees(typed).expect("check mixed contextual short-circuit scalar return");
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::measure")
        .expect("mixed contextual short-circuit scalar return lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed contextual short-circuit entry");
    let [token, plain] = entry.structural_parameters.as_slice() else {
        panic!("mixed contextual short-circuit entry retains both structural roots")
    };
    assert_eq!(entry.contract.requires.len(), 2);
    assert_eq!(entry.blocks.len(), 5);

    let mut return_obligations = Vec::new();
    let mut return_edges = Vec::new();
    for block in &entry.blocks {
        match &block.terminator {
            Terminator::Return {
                edge,
                cleanup_actions,
                ..
            } => {
                return_edges.push(*edge);
                let [
                    TerminalAffineCleanupAction::DiscardRoot(plain_cleanup),
                    TerminalAffineCleanupAction::InvokeNominal(token_cleanup),
                ] = cleanup_actions.as_slice()
                else {
                    panic!("every leaf retains the complete contextual cleanup stream")
                };
                assert_eq!(*plain_cleanup, plain.place);
                assert_eq!(token_cleanup.place, token.place);
                assert!(token_cleanup.cleanup_receiver.is_some());
                let [obligation] = token_cleanup.requirement_obligations.as_slice() else {
                    panic!("every nominal leaf owns one contextual obligation")
                };
                return_obligations.push(*obligation);
            }
            Terminator::Conditional { .. } => {}
            _ => panic!("bounded contextual return emits only decisions and value leaves"),
        }
    }
    return_edges.sort_unstable();
    return_edges.dedup();
    return_obligations.sort_unstable();
    return_obligations.dedup();
    assert_eq!(return_edges.len(), 3);
    assert_eq!(return_obligations.len(), 3);
    assert_eq!(lowered.proof_bundle.evidence.len(), 3);
    assert!(return_obligations.iter().all(|obligation| {
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == *obligation)
    }));

    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("every contextual short-circuit cleanup edge verifies independently");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("contextual short-circuit cleanup has one exact maximum path");
    assert_eq!(fixed.ceiling_units(), 11);
    validate_fixed_entry_fuel(&verified, &fixed)
        .expect("contextual short-circuit fixed-fuel certificate recomputes");
    drop(verified);
    let semantics = encode_module(&lowered.semantic_module)
        .expect("mixed contextual short-circuit module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("mixed contextual short-circuit proof encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let mut duplicated = lowered.semantic_module.clone();
    let entry = duplicated
        .machines
        .iter_mut()
        .find(|machine| machine.id == duplicated.entry)
        .expect("duplicated contextual entry");
    let mut first_obligation = None;
    for block in &mut entry.blocks {
        let Terminator::Return {
            cleanup_actions, ..
        } = &mut block.terminator
        else {
            continue;
        };
        let TerminalAffineCleanupAction::InvokeNominal(cleanup) = &mut cleanup_actions[1] else {
            unreachable!()
        };
        match first_obligation {
            Some(obligation) => {
                cleanup.requirement_obligations[0] = obligation;
                break;
            }
            None => first_obligation = Some(cleanup.requirement_obligations[0]),
        }
    }
    assert!(
        terminal_verifier::verify_module(
            &duplicated,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "one contextual obligation identity cannot be replayed on two return edges",
    );

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
            true,
            10,
        ),
        (
            [
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
            false,
            11,
        ),
    ] {
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &scalar_arguments,
            &structural_arguments,
            &mut handler,
        )
        .expect("mixed contextual short-circuit path interprets from canonical artifacts");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
        assert_eq!(measured.usage().total_units(), expected_fuel);
        assert!(measured.effects().is_empty());
    }
}

#[test]
fn contextual_scalar_cleanup_and_exact_result_use_disjoint_obligation_identities() {
    let tokens = Lexer::new(CONTEXTUAL_SCALAR_EXACT_RESULT_SOURCE)
        .tokenize()
        .expect("tokenize contextual exact scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual exact scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual exact scalar cleanup");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type contextual exact scalar cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual exact scalar cleanup source");
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::measure")
        .expect("contextual cleanup and exact scalar result lower together");

    let obligations =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("all contextual and exact-result obligations reconstruct");
    assert_eq!(obligations.len(), 5, "four cleanup goals plus exact add");
    let identities = obligations
        .iter()
        .map(|site| site.obligation.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(identities.len(), obligations.len());
    assert_eq!(lowered.proof_bundle.evidence.len(), obligations.len());
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("disjoint cleanup and exact-result proofs verify");
}
