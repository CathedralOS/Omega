use super::*;

const SCALAR_RETURN_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { value: u64; }
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::measure(token: Token) -> u64 { 7u64 }
"#;

const ORDERED_SCALAR_RETURN_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}

    data First { value: u64; }
    machine First::drop(&mut self) { FirstHelper::touch(); }
    data Second { value: u64; }
    machine Second::drop(&mut self) { SecondHelper::touch(); }

    data Root {}
    machine Root::measure(first: First, second: Second) -> u64 { 7u64 }
"#;

const SHARED_SCALAR_RETURN_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { value: u64; }
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::measure(first: Token, second: Token) -> u64 { 7u64 }
"#;

const CONTEXTUAL_SCALAR_RETURN_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::measure(first: Token, second: Token) -> u64
    requires first.ready, second.ready
    { 7u64 }
"#;

const MIXED_CONTEXTUAL_SCALAR_RETURN_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Plain { observed: bool; }

    data Root {}
    machine Root::measure(first: Token, plain: Plain, second: Token) -> u64
    requires first.ready, plain.observed, second.ready
    { 7u64 }
"#;

const MIXED_CONTEXTUAL_SCALAR_BINDINGS_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
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
    machine Token::drop(&mut self)
    requires self.ready
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
        let inverted: bool = !right;
        left && inverted
    }
"#;

const MIXED_NOMINAL_NESTED_SHORT_CIRCUIT_SCALAR_SOURCE: &str = r#"
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
        let staged: bool = left && (right || !left);
        let continued: bool = staged || (left && right);
        continued
    }
"#;

const MIXED_NOMINAL_SHARED_BOOLEAN_CONVERGENCE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { ready: bool; }
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::measure(token: Token, left: bool) -> bool {
        let staged: bool = token.ready && !left;
        staged
    }
"#;

const MIXED_SCALAR_RETURN_NOMINAL_LAST_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Plain { value: u64; }
    data Token { value: u64; }
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::measure(plain: Plain, token: Token) -> u64 { 7u64 }
"#;

const MIXED_SCALAR_RETURN_TRIVIAL_LAST_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Plain { value: u64; }
    data Token { value: u64; }
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::measure(token: Token, plain: Plain) -> u64 { 7u64 }
"#;

#[test]
fn scalar_return_materializes_value_before_nominal_cleanup_across_source_and_codec() {
    let tokens = Lexer::new(SCALAR_RETURN_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("scalar return with executable nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    assert!(matches!(entry.result, TerminalMachineResult::Scalar(_)));
    let [block] = entry.blocks.as_slice() else {
        panic!("scalar nominal entry has one block")
    };
    assert!(matches!(block.operations.as_slice(), [operation]
        if matches!(operation.kind, OperationKind::IntegerConstant { .. })));
    let Terminator::Return {
        value,
        cleanup_actions,
        ..
    } = &block.terminator
    else {
        panic!("scalar nominal entry returns a value")
    };
    let [TerminalAffineCleanupAction::InvokeNominal(cleanup)] = cleanup_actions.as_slice() else {
        panic!("scalar return carries one executable nominal cleanup")
    };
    assert_eq!(
        *value,
        block.operations[0]
            .result
            .scalar_ref()
            .expect("scalar operation result")
            .id
    );
    assert!(cleanup.cleanup_receiver.is_none());
    assert!(cleanup.requirement_obligations.is_empty());

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("scalar nominal cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn ordered_scalar_return_retains_distinct_cleanup_targets_and_helpers() {
    let tokens = Lexer::new(ORDERED_SCALAR_RETURN_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("ordered scalar return with distinct executable cleanups lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 5);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("ordered scalar entry has two roots")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("ordered scalar entry has one block")
    };
    let Terminator::Return {
        cleanup_actions, ..
    } = &block.terminator
    else {
        panic!("ordered scalar entry returns a value")
    };
    let [
        TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
        TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
    ] = cleanup_actions.as_slice()
    else {
        panic!("ordered scalar return carries two nominal cleanup actions")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_ne!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );
    let helper = |cleanup: &psi_terminal::NominalAffineCleanup| {
        let target = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == cleanup.cleanup_machine)
            .expect("cleanup target");
        let [operation] = target.blocks[0].operations.as_slice() else {
            panic!("cleanup target calls one helper")
        };
        let OperationKind::CallUnit { callee, .. } = operation.kind else {
            panic!("cleanup target operation calls a Unit helper")
        };
        callee
    };
    assert_ne!(helper(second_cleanup), helper(first_cleanup));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("ordered scalar nominal cleanups verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn ordered_scalar_return_reuses_one_shared_cleanup_target_and_helper() {
    let tokens = Lexer::new(SHARED_SCALAR_RETURN_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("ordered scalar return with one shared executable cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("shared scalar entry has two roots")
    };
    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("shared scalar entry returns a value")
    };
    let [
        TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
        TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
    ] = cleanup_actions.as_slice()
    else {
        panic!("shared scalar return carries two nominal cleanup actions")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared scalar nominal cleanups verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn mixed_scalar_return_invokes_nominal_then_discards_trivial_root() {
    let tokens = Lexer::new(MIXED_SCALAR_RETURN_NOMINAL_LAST_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("mixed scalar cleanup lowers in exact reverse-root order");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [plain, token] = entry.structural_parameters.as_slice() else {
        panic!("mixed scalar entry has two roots")
    };
    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("mixed scalar entry returns a value")
    };
    assert!(matches!(
        cleanup_actions.as_slice(),
        [
            TerminalAffineCleanupAction::InvokeNominal(cleanup),
            TerminalAffineCleanupAction::DiscardRoot(discard),
        ] if cleanup.place == token.place && *discard == plain.place
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed scalar cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn mixed_scalar_return_discards_trivial_then_invokes_nominal_root() {
    let tokens = Lexer::new(MIXED_SCALAR_RETURN_TRIVIAL_LAST_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("mixed scalar cleanup lowers in exact reverse-root order");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [token, plain] = entry.structural_parameters.as_slice() else {
        panic!("mixed scalar entry has two roots")
    };
    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("mixed scalar entry returns a value")
    };
    assert!(matches!(
        cleanup_actions.as_slice(),
        [
            TerminalAffineCleanupAction::DiscardRoot(discard),
            TerminalAffineCleanupAction::InvokeNominal(cleanup),
        ] if *discard == plain.place && cleanup.place == token.place
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed scalar cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn contextual_scalar_return_preserves_proof_context_after_result_materialization() {
    let tokens = Lexer::new(CONTEXTUAL_SCALAR_RETURN_SOURCE)
        .tokenize()
        .expect("tokenize contextual scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual scalar cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type contextual scalar cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual scalar cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("contextual scalar cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("contextual scalar entry");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("contextual scalar caller retains two roots")
    };
    assert_eq!(entry.contract.requires.len(), 2);
    let [result_operation] = entry.blocks[0].operations.as_slice() else {
        panic!("scalar result is materialized by one operation")
    };
    assert!(matches!(
        result_operation.kind,
        OperationKind::IntegerConstant { .. }
    ));
    let Terminator::Return {
        value,
        cleanup_actions,
        ..
    } = &entry.blocks[0].terminator
    else {
        panic!("contextual scalar cleanup uses the scalar return carrier")
    };
    assert_eq!(
        *value,
        result_operation.result.scalar().expect("scalar result").id
    );
    let [
        TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
        TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
    ] = cleanup_actions.as_slice()
    else {
        panic!("both contextual scalar roots retain nominal cleanup")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );
    assert_eq!(
        second_cleanup.cleanup_receiver,
        first_cleanup.cleanup_receiver
    );
    assert!(second_cleanup.cleanup_receiver.is_some());
    assert_eq!(second_cleanup.requirement_obligations.len(), 1);
    assert_eq!(first_cleanup.requirement_obligations.len(), 1);
    assert_ne!(
        second_cleanup.requirement_obligations,
        first_cleanup.requirement_obligations
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier discharges both scalar cleanup obligations");
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("contextual scalar module encodes");
    assert_eq!(
        decode_module(&semantic_bytes).unwrap(),
        lowered.semantic_module
    );
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("contextual scalar proof encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).unwrap(),
        lowered.proof_bundle
    );

    let mut missing = lowered.proof_bundle.clone();
    missing.evidence.pop();
    assert!(
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn mixed_contextual_scalar_return_rebases_compact_nominal_proofs_to_full_roots() {
    let tokens = Lexer::new(MIXED_CONTEXTUAL_SCALAR_RETURN_SOURCE)
        .tokenize()
        .expect("tokenize mixed contextual scalar cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed contextual scalar cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed contextual scalar cleanup");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type mixed contextual scalar cleanup source");
    let checked = lower_typed_trees(typed).expect("check mixed contextual scalar cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("mixed contextual scalar cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("mixed contextual scalar entry");
    let [first, plain, second] = entry.structural_parameters.as_slice() else {
        panic!("full scalar entry retains both nominal roots and the no-code root")
    };
    let caller_roots = entry
        .contract
        .requires
        .iter()
        .map(|requirement| match requirement {
            Proposition::Equal(_, ScalarTerm::BooleanField { root, .. }) => *root,
            _ => panic!("bounded caller requirement remains a direct Boolean field"),
        })
        .collect::<Vec<_>>();
    assert_eq!(caller_roots.len(), 3);
    assert!(caller_roots.contains(&first.place));
    assert!(caller_roots.contains(&plain.place));
    assert!(caller_roots.contains(&second.place));

    let Terminator::Return {
        cleanup_actions, ..
    } = &entry.blocks[0].terminator
    else {
        panic!("mixed contextual entry returns a scalar")
    };
    let [
        TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
        TerminalAffineCleanupAction::DiscardRoot(plain_cleanup),
        TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
    ] = cleanup_actions.as_slice()
    else {
        panic!("mixed contextual cleanup retains complete reverse-authored order")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(*plain_cleanup, plain.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(
        second_cleanup.cleanup_machine, first_cleanup.cleanup_machine,
        "both nominal roots reuse one contextual cleanup target",
    );
    assert_eq!(
        second_cleanup.cleanup_receiver,
        first_cleanup.cleanup_receiver
    );
    let receiver = second_cleanup
        .cleanup_receiver
        .expect("shared cleanup target retains one proof-only receiver");
    assert!(
        ![first.place, plain.place, second.place].contains(&receiver),
        "proof-only receiver does not alias the restored full entry roots",
    );
    assert_eq!(second_cleanup.requirement_obligations.len(), 1);
    assert_eq!(first_cleanup.requirement_obligations.len(), 1);
    assert_ne!(
        second_cleanup.requirement_obligations,
        first_cleanup.requirement_obligations
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("rebased mixed contextual scalar cleanup verifies");
    let semantics = encode_module(&lowered.semantic_module).expect("mixed semantic module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("mixed proof bundle encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let mut swapped = lowered.semantic_module.clone();
    let entry = swapped
        .machines
        .iter_mut()
        .find(|machine| machine.id == swapped.entry)
        .expect("tampered mixed contextual entry");
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut entry.blocks[0].terminator
    else {
        unreachable!()
    };
    let [
        TerminalAffineCleanupAction::InvokeNominal(second_cleanup),
        TerminalAffineCleanupAction::DiscardRoot(_),
        TerminalAffineCleanupAction::InvokeNominal(first_cleanup),
    ] = cleanup_actions.as_mut_slice()
    else {
        unreachable!()
    };
    std::mem::swap(
        &mut second_cleanup.requirement_obligations,
        &mut first_cleanup.requirement_obligations,
    );
    assert!(
        psi_terminal_verifier::verify_module(
            &swapped,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "root-specific contextual obligations cannot be swapped across the no-code action",
    );
}

#[test]
fn mixed_contextual_scalar_return_materializes_branch_free_bindings_before_cleanup() {
    let tokens = Lexer::new(MIXED_CONTEXTUAL_SCALAR_BINDINGS_SOURCE)
        .tokenize()
        .expect("tokenize mixed contextual scalar bindings");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed contextual scalar bindings");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed contextual scalar bindings");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type mixed contextual scalar bindings source");
    let checked = lower_typed_trees(typed).expect("check mixed contextual scalar bindings source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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
    let max_cleanup_obligation = lowered
        .proof_bundle
        .evidence
        .iter()
        .filter(|evidence| evidence.obligation.get() <= 2)
        .map(|evidence| evidence.obligation.get())
        .max()
        .expect("both contextual cleanup obligations remain present");
    assert!(ready.id.get() > max_cleanup_obligation);
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
    assert_eq!(entry.contract.requires.len(), 3);
    assert_eq!(
        lowered.proof_bundle.evidence.len(),
        2,
        "proof obligations remain disjoint from the later value-operation namespace",
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed contextual scalar bindings verify");
    let semantics = encode_module(&lowered.semantic_module).expect("binding module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("binding proof encodes");
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);

    let structural_arguments = [first, plain, second].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    let mut handler = AcceptTerminalEffects;
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &structural_arguments,
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
    let tokens = Lexer::new(MIXED_CONTEXTUAL_SCALAR_INPUTS_SOURCE)
        .tokenize()
        .expect("tokenize mixed contextual scalar inputs");
    let syntax = parse_syntax_trees(&tokens).expect("parse mixed contextual scalar inputs");
    let resolved = lower_syntax_trees(&syntax).expect("resolve mixed contextual scalar inputs");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type mixed contextual scalar inputs source");
    let checked = lower_typed_trees(typed).expect("check mixed contextual scalar inputs source");
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
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed contextual scalar inputs verify");
    let semantics = encode_module(&lowered.semantic_module).expect("input module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("input proof encodes");
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
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &scalar_arguments,
        &structural_arguments,
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
    let tokens = Lexer::new(MIXED_NOMINAL_SHORT_CIRCUIT_SCALAR_SOURCE)
        .tokenize()
        .expect("tokenize mixed nominal short-circuit scalar return");
    let syntax =
        parse_syntax_trees(&tokens).expect("parse mixed nominal short-circuit scalar return");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve mixed nominal short-circuit scalar return");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type mixed nominal short-circuit scalar return");
    let checked =
        lower_typed_trees(typed).expect("check mixed nominal short-circuit scalar return");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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
        Some(psi_terminal::Operation {
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
    assert!(token_cleanup.cleanup_receiver.is_none());
    assert!(token_cleanup.requirement_obligations.is_empty());
    let cleanup_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == token_cleanup.cleanup_machine)
        .expect("nominal cleanup target remains in the terminal closure");
    assert!(matches!(
        cleanup_target.blocks[0].operations.as_slice(),
        [psi_terminal::Operation {
            kind: OperationKind::CallUnit { .. },
            ..
        }]
    ));
    assert!(lowered.proof_bundle.evidence.is_empty());

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("mixed nominal short-circuit cleanup verifies on every leaf");
    let semantics = encode_module(&lowered.semantic_module)
        .expect("mixed nominal short-circuit module encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle)
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
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &scalar_arguments,
            &structural_arguments,
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
    let tokens = Lexer::new(MIXED_NOMINAL_NESTED_SHORT_CIRCUIT_SCALAR_SOURCE)
        .tokenize()
        .expect("tokenize nested nominal short-circuit scalar return");
    let syntax =
        parse_syntax_trees(&tokens).expect("parse nested nominal short-circuit scalar return");
    let resolved =
        lower_syntax_trees(&syntax).expect("resolve nested nominal short-circuit scalar return");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type nested nominal short-circuit scalar return");
    let checked =
        lower_typed_trees(typed).expect("check nested nominal short-circuit scalar return");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("nested nominal short-circuit cleanup verifies on every leaf");
    let semantics = encode_module(&lowered.semantic_module)
        .expect("nested nominal short-circuit module encodes");
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("nested nominal short-circuit proof encodes");
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
    let tokens = Lexer::new(MIXED_NOMINAL_SHARED_BOOLEAN_CONVERGENCE_SOURCE)
        .tokenize()
        .expect("tokenize shared nominal Boolean convergence");
    let syntax = parse_syntax_trees(&tokens).expect("parse shared nominal Boolean convergence");
    let resolved = lower_syntax_trees(&syntax).expect("resolve shared nominal Boolean convergence");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type shared nominal Boolean convergence");
    let checked = lower_typed_trees(typed).expect("check shared nominal Boolean convergence");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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
                OperationKind::BooleanStructuralField { source, field }
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

    let verified = psi_terminal_verifier::verify_module(
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
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("shared nominal Boolean convergence proof encodes");
    let structural_arguments = [token].map(|parameter| TerminalStructuralValue {
        opaque_identity: parameter.place.get(),
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    let mut handler = AcceptTerminalEffects;
    let missing = interpret_terminal_artifact_with_structural_boolean_fields_measured(
        &semantics,
        &proof,
        &AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(true)],
        &structural_arguments,
        &[],
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
        let measured = interpret_terminal_artifact_with_structural_boolean_fields_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(left)],
            &structural_arguments,
            &[TerminalStructuralBooleanFieldValue {
                argument_index: 0,
                field: ready.id,
                value: ready_value,
            }],
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
