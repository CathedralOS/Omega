use psi_core::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType};
use psi_proof_kernel::{AdmissionProfile, EvidenceRoute, ProofRule};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachineResult, Terminator,
};
use psi_terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_interpreter::{
    AcceptTerminalEffects, TerminalArtifactInterpretError, TerminalExecutionResult,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralValue, interpret_terminal_artifact_with_effect_handler_measured,
    interpret_terminal_artifact_with_structural_boolean_fields_measured,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const SCALAR_SOURCE: &str = r#"
    data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.ready
    {}
"#;

const FINITE_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; audited: bool; armed: bool; }
    machine Token::drop(&mut self)
    requires
        self.armed;
        self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires
        token.armed;
        token.audited;
        token.ready
    {}
"#;

const CALLER_ONLY_CONTEXTUAL_SOURCE: &str = r#"
    data Token { observed: bool; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.observed
    {}
"#;

const TWO_ROOT_SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const TWO_ROOT_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::enter(first: Token, second: Token)
    requires first.ready, second.ready
    {}
"#;

const TWO_ROOT_ONE_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}
    data ThirdHelper {}
    machine ThirdHelper::touch() {}

    data First {}
    machine First::drop(&mut self) {
        FirstHelper::touch();
        SecondHelper::touch();
        ThirdHelper::touch();
    }
    data Second {}
    machine Second::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_ROOT_TWO_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}

    data First {}
    machine First::drop(&mut self) { FirstHelper::touch(); }
    data Second {}
    machine Second::drop(&mut self) { SecondHelper::touch(); }

    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_ROOT_SHARED_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const THREE_ROOT_DISTINCT_SOURCE: &str = r#"
    data First {}
    machine First::drop(&mut self) {}
    data Second {}
    machine Second::drop(&mut self) {}
    data Third {}
    machine Third::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: First, second: Second, third: Third) {}
"#;

const THREE_ROOT_SHARED_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Token, second: Token, third: Token) {}
"#;

const EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { ready: bool; padding: u8; }
    machine Token::drop(&mut self)
    requires self.ready
    {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(first: Token, second: Token)
    requires second.ready, first.ready
    {}
"#;

const TWO_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const THREE_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}
    data Third {}
    machine Third::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
        Third::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

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

const MIXED_NOMINAL_SHARED_INTEGER_COMPARISON_CONVERGENCE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::measure(
        token: Token,
        input: u64 in Wrapping,
        small: u8,
        enabled: bool
    ) -> bool
    requires input <= 255u64, small <= 254u8, small <= 127u8, small <= 7u8
    {
        let staged: bool = ((((input + 1u64) < 4u64) || ((~input) < 1u64) || (input <= 9u64))
            && ((small as u16) < 5u16))
            && ((input as u8) < 5u8)
            && ((small + 1u8) < 6u8)
            && ((127u8 - small) < 125u8)
            && ((small * 2u8) < 10u8)
            && ((small / 2u8) < 3u8)
            && ((small % 2u8) <= 1u8)
            && ((small >> small) < 1u8)
            && (input == 3u64)
            && enabled;
        staged
    }
"#;

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

#[test]
fn mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return() {
    let tokens = Lexer::new(MIXED_NOMINAL_SHARED_INTEGER_COMPARISON_CONVERGENCE_SOURCE)
        .tokenize()
        .expect("tokenize shared integer-comparison convergence");
    let syntax = parse_syntax_trees(&tokens).expect("parse shared integer-comparison convergence");
    let resolved = lower_syntax_trees(&syntax).expect("resolve shared integer convergence");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type shared integer convergence");
    let checked = lower_typed_trees(typed).expect("check shared integer convergence");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("shared integer-comparison convergence lowers");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("shared integer convergence entry");
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerLessThan { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::WrappingIntegerAdd { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerBitwiseNot { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerWiden { .. }))
    }));
    let cast_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerExactCast { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the guarded exact cast");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == cast_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_divide_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerDivide { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact division by a nonzero constant");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_divide_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_remainder_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains exact remainder by a nonzero constant");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_remainder_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_shift_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerShiftRight { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact right shift");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_shift_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_multiply_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerMultiply { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact multiplication");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_multiply_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_subtract_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the bounded exact subtraction");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_subtract_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let exact_add_obligation = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("shared convergence retains the proven exact addition");
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == exact_add_obligation
            && matches!(
                evidence.route,
                psi_proof_kernel::EvidenceRoute::CertificateDerived(_)
            )
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerLessOrEqual { .. }))
    }));
    assert!(entry.blocks.iter().any(|block| {
        block
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::IntegerEqual { .. }))
    }));
    assert_eq!(
        entry
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Terminator::Return { .. }))
            .count(),
        1
    );
    let (convergence, control) = entry
        .blocks
        .split_last()
        .expect("shared integer convergence has one cleanup tail");
    assert!(control.iter().any(|block| {
        matches!(
            block.terminator,
            Terminator::Jump { target, .. } if target == convergence.id
        )
    }));

    let verified = psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared integer convergence verifies");
    let fixed = derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry)
        .expect("shared integer convergence has fixed fuel");
    validate_fixed_entry_fuel(&verified, &fixed)
        .expect("shared integer convergence fuel recomputes");
    drop(verified);
    let semantics =
        encode_module(&lowered.semantic_module).expect("shared integer convergence encodes");
    assert_eq!(decode_module(&semantics).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle)
        .expect("shared integer convergence proof encodes");
    let mut missing_cast_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != cast_obligation);
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == cast_obligation
    ));
    let mut changed_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_bound.entry;
    let entry_contract = &mut changed_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let [
        Proposition::LessOrEqual(subject, _),
        Proposition::LessOrEqual(_, _),
        Proposition::LessOrEqual(_, _),
        Proposition::LessOrEqual(_, _),
    ] = entry_contract.requires.as_slice()
    else {
        panic!("shared convergence retains all unsigned upper-bound premises")
    };
    entry_contract.requires[0] = Proposition::LessOrEqual(
        subject.clone(),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            IntegerValue::Unsigned(254),
        )
        .unwrap(),
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == cast_obligation
    ));
    let mut missing_exact_add_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_exact_add_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_add_obligation);
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_exact_add_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_add_obligation
    ));
    let mut changed_add_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_add_bound.entry;
    let entry_contract = &mut changed_add_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let Proposition::LessOrEqual(add_subject, _) = &entry_contract.requires[3] else {
        panic!("shared convergence retains the exact-add upper-bound premise")
    };
    entry_contract.requires[3] = Proposition::LessOrEqual(
        add_subject.clone(),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(253),
        )
        .unwrap(),
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_add_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == exact_add_obligation
    ));
    let mut missing_exact_subtract_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_exact_subtract_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_subtract_obligation);
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_exact_subtract_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_subtract_obligation
    ));
    let mut missing_exact_multiply_proof =
        decode_proof_bundle(&proof).expect("decode shared proof");
    missing_exact_multiply_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_multiply_obligation);
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_exact_multiply_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_multiply_obligation
    ));
    let mut changed_exact_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_exact_bound.entry;
    let entry_contract = &mut changed_exact_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let Proposition::LessOrEqual(exact_subject, _) = &entry_contract.requires[2] else {
        panic!("shared convergence retains the subtract/multiply upper-bound premise")
    };
    entry_contract.requires[2] = Proposition::LessOrEqual(
        exact_subject.clone(),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(126),
        )
        .unwrap(),
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_exact_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == exact_subtract_obligation || obligation == exact_multiply_obligation
    ));
    for obligation in [exact_divide_obligation, exact_remainder_obligation] {
        let mut missing_proof = decode_proof_bundle(&proof).expect("decode shared proof");
        missing_proof
            .evidence
            .retain(|evidence| evidence.obligation != obligation);
        assert!(matches!(
            psi_terminal_verifier::verify_module(
                &decode_module(&semantics).expect("decode shared semantics"),
                &missing_proof,
                &AdmissionProfile::default(),
            ),
            Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
                if missing == obligation
        ));
    }
    let mut missing_shift_proof = decode_proof_bundle(&proof).expect("decode shared proof");
    missing_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != exact_shift_obligation);
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &decode_module(&semantics).expect("decode shared semantics"),
            &missing_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == exact_shift_obligation
    ));
    let mut changed_shift_bound = decode_module(&semantics).expect("decode shared semantics");
    let changed_entry = changed_shift_bound.entry;
    let entry_contract = &mut changed_shift_bound
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed_entry)
        .expect("changed shared entry")
        .contract;
    let Proposition::LessOrEqual(shift_subject, _) = &entry_contract.requires[1] else {
        panic!("shared convergence retains the exact-shift count premise")
    };
    entry_contract.requires[1] = Proposition::LessOrEqual(
        shift_subject.clone(),
        ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            IntegerValue::Unsigned(6),
        )
        .unwrap(),
    );
    assert!(matches!(
        psi_terminal_verifier::verify_module(
            &changed_shift_bound,
            &decode_proof_bundle(&proof).expect("decode unchanged shared proof"),
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation,
            ..
        }) if obligation == exact_shift_obligation
    ));
    let [token] = entry.structural_parameters.as_slice() else {
        panic!("shared integer convergence retains its cleanup root")
    };
    let structural_arguments = [TerminalStructuralValue {
        opaque_identity: token.place.get(),
        structural_type: token.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    for (input, small, enabled) in [
        (3_u128, 4_u128, false),
        (3, 4, true),
        (3, 5, true),
        (4, 4, true),
        (10, 4, true),
    ] {
        let mask = u128::from(u64::MAX);
        let bitwise_not = (!input) & mask;
        let wrapped_add = (input + 1) & mask;
        let mut handler = AcceptTerminalEffects;
        let measured = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    value: IntegerValue::Unsigned(input),
                },
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(small),
                },
                TerminalScalarValue::Boolean(enabled),
            ],
            &structural_arguments,
            &mut handler,
        )
        .expect("shared integer convergence interprets");
        assert_eq!(
            measured.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                ((wrapped_add < 4) || (bitwise_not < 1) || (input <= 9))
                    && small < 5
                    && input < 5
                    && small + 1 < 6
                    && 127 - small < 125
                    && small * 2 < 10
                    && small / 2 < 3
                    && small % 2 <= 1
                    && (small >> small) < 1
                    && input == 3
                    && enabled
            ))
        );
    }
}

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
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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

    psi_terminal_verifier::verify_module(
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
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
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

    let verified = psi_terminal_verifier::verify_module(
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
        psi_terminal_verifier::verify_module(
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
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::measure")
        .expect("contextual cleanup and exact scalar result lower together");

    let obligations =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("all contextual and exact-result obligations reconstruct");
    assert_eq!(obligations.len(), 5, "four cleanup goals plus exact add");
    let identities = obligations
        .iter()
        .map(|site| site.obligation.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(identities.len(), obligations.len());
    assert_eq!(lowered.proof_bundle.evidence.len(), obligations.len());
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("disjoint cleanup and exact-result proofs verify");
}

#[test]
fn empty_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("empty nominal cleanup lowers");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "the cleanup target is part of the executable terminal closure"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("nominal cleanup source slice has one structural root")
    };
    assert_eq!(root.multiplicity, StructuralMultiplicity::Affine);
    assert!(root.qualifications.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("nominal cleanup source slice has one block")
    };
    assert!(block.operations.is_empty());
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("expected executable nominal cleanup return")
    };
    assert_eq!(cleanups[0].place, root.place);
    assert_eq!(cleanups[0].structural_type, root.structural_type);

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target machine");
    assert_eq!(target.attachment, Some(cleanups[0].structural_type));
    assert!(target.structural_parameters.is_empty());
    assert!(target.blocks[0].operations.is_empty());
    assert!(matches!(
        &target.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "nominal cleanup target identity is canonical artifact data"
    );
}

#[test]
fn contextual_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("contextual nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("contextual cleanup caller has one structural parameter")
    };
    let [caller_requirement] = entry.contract.requires.as_slice() else {
        panic!("contextual cleanup caller retains one required premise")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("contextual cleanup caller has one block")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("contextual cleanup uses the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("contextual cleanup has one action")
    };
    let receiver = cleanup
        .cleanup_receiver
        .expect("contextual cleanup carries a proof-only receiver root");
    let [obligation] = cleanup.requirement_obligations.as_slice() else {
        panic!("contextual cleanup carries one requirement obligation")
    };
    assert_ne!(receiver, parameter.place);
    assert_eq!(lowered.proof_bundle.evidence.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence[0].obligation, *obligation);

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target");
    assert!(target.structural_parameters.is_empty());
    assert!(target.structural_places.is_empty());
    let [target_requirement] = target.contract.requires.as_slice() else {
        panic!("cleanup target retains one contextual requirement")
    };
    let psi_core::Proposition::Equal(target_left, target_right) = target_requirement else {
        panic!("target contextual requirement is an equality")
    };
    assert_eq!(target_left, &psi_core::ScalarTerm::boolean(true));
    let psi_core::ScalarTerm::BooleanField {
        root: target_root,
        path: target_path,
    } = target_right
    else {
        panic!("target contextual requirement names its Boolean field")
    };
    let [psi_core::CanonicalStructuralPathSegment::Field(target_field)] = target_path.as_slice()
    else {
        panic!("target contextual requirement names one direct Boolean field")
    };
    assert_eq!(*target_root, receiver);
    let psi_core::Proposition::Equal(caller_left, caller_right) = caller_requirement else {
        panic!("caller contextual requirement is an equality")
    };
    assert_eq!(caller_left, &psi_core::ScalarTerm::boolean(true));
    assert_eq!(
        caller_right,
        &psi_core::ScalarTerm::boolean_field(parameter.place, *target_field),
        "the caller assumption is the cleanup target premise rebased to the owned root",
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier discharges contextual cleanup from the caller requirement");
    let bytes = encode_module(&lowered.semantic_module).expect("contextual module encodes");
    assert_eq!(
        decode_module(&bytes).expect("contextual module decodes"),
        lowered.semantic_module,
        "contextual cleanup premise and obligation are canonical terminal data",
    );
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("contextual proof bundle decodes"),
        lowered.proof_bundle,
        "contextual cleanup evidence is canonical proof-artifact data",
    );
}

#[test]
fn finite_contextual_nominal_cleanup_preserves_caller_superset_and_canonical_artifacts() {
    let tokens = Lexer::new(FINITE_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize finite contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse finite contextual cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve finite contextual cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type finite contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check finite contextual cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("finite contextual nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("finite contextual cleanup caller has one structural parameter")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("finite contextual cleanup caller has one block")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("finite contextual cleanup uses the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("finite contextual cleanup has one action")
    };
    let receiver = cleanup
        .cleanup_receiver
        .expect("finite contextual cleanup carries a proof-only receiver root");
    assert_ne!(receiver, parameter.place);
    assert_eq!(
        cleanup
            .requirement_obligations
            .iter()
            .map(|obligation| obligation.get())
            .collect::<Vec<_>>(),
        vec![1, 2],
        "cleanup obligations are stable and dense in target-clause order",
    );

    let token_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
        .expect("Token terminal type");
    let StructuralTypeShape::Record { fields } = &token_type.shape else {
        panic!("Token is a record")
    };
    let field = |identity: &str| {
        fields
            .iter()
            .find(|field| field.identity == identity)
            .unwrap_or_else(|| panic!("{identity} terminal field"))
            .id
    };
    let ready = field("ready");
    let armed = field("armed");
    let audited = field("audited");
    let caller_requires = vec![
        psi_core::Proposition::Equal(
            psi_core::ScalarTerm::boolean(true),
            psi_core::ScalarTerm::boolean_field(parameter.place, ready),
        ),
        psi_core::Proposition::Equal(
            psi_core::ScalarTerm::boolean(true),
            psi_core::ScalarTerm::boolean_field(parameter.place, audited),
        ),
        psi_core::Proposition::Equal(
            psi_core::ScalarTerm::boolean(true),
            psi_core::ScalarTerm::boolean_field(parameter.place, armed),
        ),
    ];
    assert_eq!(
        entry.contract.requires, caller_requires,
        "the full caller set is canonically ordered by terminal field identity",
    );

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target");
    assert!(target.structural_parameters.is_empty());
    assert!(target.structural_places.is_empty());
    assert_eq!(
        target.contract.requires,
        vec![
            psi_core::Proposition::Equal(
                psi_core::ScalarTerm::boolean(true),
                psi_core::ScalarTerm::boolean_field(receiver, ready),
            ),
            psi_core::Proposition::Equal(
                psi_core::ScalarTerm::boolean(true),
                psi_core::ScalarTerm::boolean_field(receiver, armed),
            ),
        ],
        "the cleanup target retains only its canonical requirement subset",
    );
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);
    for (obligation_index, evidence) in lowered.proof_bundle.evidence.iter().enumerate() {
        assert_eq!(
            evidence.obligation,
            cleanup.requirement_obligations[obligation_index]
        );
        let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
            panic!("contextual cleanup evidence is certificate-derived")
        };
        let assumption_index = [0, 2][obligation_index];
        assert_eq!(
            certificate.proof.conclusion,
            caller_requires[assumption_index]
        );
        assert!(matches!(
            certificate.proof.rule,
            ProofRule::Assumption { index: assumption } if assumption == assumption_index
        ));
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier discharges the finite cleanup subset from the caller superset");
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("finite contextual module encodes");
    assert_eq!(
        decode_module(&semantic_bytes).expect("finite contextual module decodes"),
        lowered.semantic_module,
        "finite contextual cleanup semantic data is canonical",
    );
    let proof_bytes =
        encode_proof_bundle(&lowered.proof_bundle).expect("finite contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("finite contextual proof bundle decodes"),
        lowered.proof_bundle,
        "finite contextual cleanup proof data is canonical",
    );
}

#[test]
fn caller_only_contextual_fact_does_not_invent_a_cleanup_receiver_or_obligation() {
    let tokens = Lexer::new(CALLER_ONLY_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize caller-only contextual cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse caller-only contextual cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve caller-only contextual cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type caller-only contextual cleanup source");
    let checked = lower_typed_trees(typed).expect("check caller-only contextual cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("caller-only contextual nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("caller-only contextual cleanup has one structural parameter")
    };
    let [caller_requirement] = entry.contract.requires.as_slice() else {
        panic!("caller-only contextual fact is retained")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("caller-only contextual cleanup has one block")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("caller-only contextual cleanup uses the nominal return carrier")
    };
    let [cleanup] = cleanups.as_slice() else {
        panic!("caller-only contextual cleanup has one action")
    };
    assert!(cleanup.cleanup_receiver.is_none());
    assert!(cleanup.requirement_obligations.is_empty());
    assert!(lowered.proof_bundle.evidence.is_empty());

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target");
    assert!(target.contract.requires.is_empty());
    let psi_core::Proposition::Equal(
        psi_core::ScalarTerm::Boolean(true),
        psi_core::ScalarTerm::BooleanField { root, .. },
    ) = caller_requirement
    else {
        panic!("caller-only contextual fact retains its Boolean-field shape")
    };
    assert_eq!(*root, parameter.place);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts a caller-only fact without a cleanup obligation");
    let semantic_bytes =
        encode_module(&lowered.semantic_module).expect("caller-only contextual module encodes");
    assert_eq!(
        decode_module(&semantic_bytes).expect("caller-only contextual module decodes"),
        lowered.semantic_module,
    );
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("caller-only contextual proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("caller-only contextual proof bundle decodes"),
        lowered.proof_bundle,
    );
}

#[test]
fn wide_mixed_primitive_record_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SCALAR_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("wide flat scalar nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let cleanup_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanups[0].structural_type)
        .expect("cleanup structural type");
    let StructuralTypeShape::Record { fields } = &cleanup_type.shape else {
        panic!("cleanup type remains a record")
    };
    let [flag, tag, delta, payload, address] = fields.as_slice() else {
        panic!("bounded cleanup record retains all five fields")
    };
    for (field, identity, scalar_type) in [
        (flag, "flag", ScalarType::Boolean),
        (
            tag,
            "tag",
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
        ),
        (
            delta,
            "delta",
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16")),
        ),
        (
            payload,
            "payload",
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")),
        ),
        (
            address,
            "address",
            ScalarType::Integer(IntegerType::address(64).expect("addr")),
        ),
    ] {
        assert_eq!(field.identity, identity);
        assert!(!field.relevance.is_erased());
        let StructuralFieldType::Scalar(actual) = &field.field_type else {
            panic!("wide cleanup record retains scalar carriers")
        };
        assert_eq!(*actual, scalar_type);
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts wide flat scalar nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the primitive field and nominal cleanup identity are canonical artifact data"
    );
}

#[test]
fn two_nominal_roots_cleanup_in_reverse_parameter_order_and_may_share_a_target() {
    let tokens = Lexer::new(TWO_ROOT_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two nominal roots lower");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "same-type roots share one exact cleanup target"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected ordered nominal cleanup return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("both roots require nominal cleanup")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(second_cleanup.structural_type, second.structural_type);
    assert_eq!(first_cleanup.structural_type, first.structural_type);
    assert_eq!(
        second_cleanup.cleanup_machine, first_cleanup.cleanup_machine,
        "same-type roots reuse the same exact cleanup target"
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts ordered two-root nominal cleanup");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
}

#[test]
fn contextual_multi_root_nominal_cleanup_crosses_source_codec_and_verifier() {
    let tokens = Lexer::new(TWO_ROOT_CONTEXTUAL_SOURCE)
        .tokenize()
        .expect("tokenize contextual two-root cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual two-root cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual two-root cleanup");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type contextual two-root cleanup source");
    let checked = lower_typed_trees(typed).expect("check contextual two-root cleanup source");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("contextual two-root cleanup lowers");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("terminal entry");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("contextual caller retains two owned roots")
    };
    assert_eq!(entry.contract.requires.len(), 2);
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("contextual multi-root cleanup uses nominal return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("contextual multi-root cleanup retains both actions")
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
    .expect("verifier independently discharges both root-specific cleanup goals");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).expect("proof bundle decodes"),
        lowered.proof_bundle
    );
}

#[test]
fn two_nominal_roots_allow_one_executable_cleanup_in_reverse_order() {
    let tokens = Lexer::new(TWO_ROOT_ONE_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("one executable cleanup in a two-root list lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 6);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected ordered nominal cleanup return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("both roots require nominal cleanup")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    let second_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == second_cleanup.cleanup_machine)
        .expect("second cleanup target");
    let first_target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == first_cleanup.cleanup_machine)
        .expect("first cleanup target");
    assert!(second_target.blocks[0].operations.is_empty());
    let [first_helper_call, second_helper_call, third_helper_call] =
        first_target.blocks[0].operations.as_slice()
    else {
        panic!("exactly one cleanup body retains all three ordered helper calls")
    };
    let helper_callees =
        [first_helper_call, second_helper_call, third_helper_call].map(|operation| {
            let OperationKind::CallUnit {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } = &operation.kind
            else {
                panic!("cleanup helper operation remains an ordinary Unit call")
            };
            assert!(structural_arguments.is_empty());
            assert!(claim_transfers.is_empty());
            assert!(requirement_obligations.is_empty());
            assert!(crash_continuations.is_empty());
            *callee
        });
    assert_ne!(helper_callees[0], helper_callees[1]);
    assert_ne!(helper_callees[0], helper_callees[2]);
    assert_ne!(helper_callees[1], helper_callees[2]);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts one executable cleanup in an ordered list");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
}

#[test]
fn two_nominal_roots_run_distinct_executable_cleanups_in_reverse_order() {
    let tokens = Lexer::new(TWO_ROOT_TWO_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two distinct executable cleanup actions lower");

    assert_eq!(lowered.semantic_module.machines.len(), 5);
    let entry = &lowered.semantic_module.machines[0];
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(
        [cleanups[0].place, cleanups[1].place],
        [second.place, first.place]
    );
    let helper_ids = cleanups
        .iter()
        .map(|cleanup| {
            let target = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == cleanup.cleanup_machine)
                .expect("cleanup target");
            let [operation] = target.blocks[0].operations.as_slice() else {
                panic!("each cleanup body has one helper call")
            };
            let OperationKind::CallUnit { callee, .. } = operation.kind else {
                panic!("cleanup helper call")
            };
            callee
        })
        .collect::<Vec<_>>();
    assert_ne!(helper_ids[0], helper_ids[1]);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("two executable cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn two_nominal_roots_may_repeat_one_executable_cleanup_target_and_helper() {
    let tokens = Lexer::new(TWO_ROOT_SHARED_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("shared executable cleanup target lowers");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        3,
        "caller, shared cleanup target, and shared helper form the exact closure"
    );
    let entry = &lowered.semantic_module.machines[0];
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(cleanups[0].cleanup_machine, cleanups[1].cleanup_machine);
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("shared cleanup target");
    assert_eq!(target.blocks[0].operations.len(), 1);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared executable cleanup target verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn contextual_roots_may_share_one_executable_cleanup_target_and_helper() {
    let tokens = Lexer::new(CONTEXTUAL_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize contextual executable cleanup");
    let syntax = parse_syntax_trees(&tokens).expect("parse contextual executable cleanup");
    let resolved = lower_syntax_trees(&syntax).expect("resolve contextual executable cleanup");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type contextual executable cleanup");
    let checked = lower_typed_trees(typed).expect("check contextual executable cleanup");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("contextual executable cleanup lowers");

    let entry = &lowered.semantic_module.machines[0];
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("contextual executable caller retains two roots")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("contextual executable cleanup uses nominal return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("contextual executable cleanup retains both actions")
    };
    assert_eq!(
        [second_cleanup.place, first_cleanup.place],
        [second.place, first.place]
    );
    assert_eq!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );
    assert_eq!(
        second_cleanup.cleanup_receiver,
        first_cleanup.cleanup_receiver
    );
    assert!(second_cleanup.cleanup_receiver.is_some());
    assert_ne!(
        second_cleanup.requirement_obligations,
        first_cleanup.requirement_obligations
    );
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == second_cleanup.cleanup_machine)
        .expect("shared contextual cleanup target");
    assert_eq!(target.contract.requires.len(), 1);
    assert_eq!(target.blocks[0].operations.len(), 1);
    assert_eq!(lowered.proof_bundle.evidence.len(), 2);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("contextual executable cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
    let proof_bytes = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    assert_eq!(
        decode_proof_bundle(&proof_bytes).unwrap(),
        lowered.proof_bundle
    );
    let structural_arguments = [
        TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: first.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 2,
            structural_type: second.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        },
    ];
    let mut handler = AcceptTerminalEffects;
    let measured = interpret_terminal_artifact_with_effect_handler_measured(
        &bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        &structural_arguments,
        &mut handler,
    )
    .expect("contextual executable cleanup interprets from canonical artifact sections");
    assert_eq!(measured.value(), TerminalExecutionResult::Unit);
    assert_eq!(measured.usage().total_units(), 7);
    assert!(measured.effects().is_empty());
}

#[test]
fn three_distinct_nominal_roots_cross_source_codec_and_verifier_in_reverse_order() {
    let tokens = Lexer::new(THREE_ROOT_DISTINCT_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("three distinct cleanup targets lower");

    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("three source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    let [third_cleanup, second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("all three roots require nominal cleanup")
    };
    assert_eq!(
        [
            third_cleanup.place,
            second_cleanup.place,
            first_cleanup.place
        ],
        [third.place, second.place, first.place]
    );
    assert_ne!(
        third_cleanup.cleanup_machine,
        second_cleanup.cleanup_machine
    );
    assert_ne!(third_cleanup.cleanup_machine, first_cleanup.cleanup_machine);
    assert_ne!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three distinct ordered cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn three_nominal_roots_may_share_one_executable_target_and_helper() {
    let tokens = Lexer::new(THREE_ROOT_SHARED_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("three shared executable cleanup actions lower");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("three source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![third.place, second.place, first.place]
    );
    assert!(
        cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == cleanups[0].cleanup_machine)
    );
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("shared cleanup target");
    assert_eq!(target.blocks[0].operations.len(), 1);

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three shared executable cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn one_call_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(EXECUTABLE_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("one-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [call] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target must contain exactly one call")
    };
    assert_eq!(call.result, OperationResult::Unit);
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &call.kind
    else {
        panic!("cleanup operation must be an ordinary Unit call")
    };
    assert!(structural_arguments.is_empty());
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .expect("cleanup helper");
    assert_ne!(helper.id, target.id);
    assert_ne!(helper.id, entry.id);
    assert!(helper.structural_parameters.is_empty());
    assert!(helper.blocks[0].operations.is_empty());
    assert!(matches!(
        &helper.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact executable nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the three-machine cleanup closure is canonical artifact data"
    );
}

#[test]
fn two_call_nominal_cleanup_preserves_source_order_through_codec_and_verifier() {
    let tokens = Lexer::new(TWO_CALL_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [first_call, second_call] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target must contain exactly two calls")
    };
    let callees = [first_call, second_call].map(|operation| {
        assert_eq!(operation.result, OperationResult::Unit);
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = &operation.kind
        else {
            panic!("cleanup operation must be an ordinary Unit call")
        };
        assert!(structural_arguments.is_empty());
        assert!(claim_transfers.is_empty());
        assert!(requirement_obligations.is_empty());
        assert!(crash_continuations.is_empty());
        *callee
    });
    assert_ne!(callees[0], callees[1]);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![entry.id, target.id, callees[0], callees[1]],
        "the exact closure retains source call order"
    );
    for callee in callees {
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == callee)
            .expect("cleanup helper");
        assert!(helper.structural_parameters.is_empty());
        assert!(helper.blocks[0].operations.is_empty());
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact ordered two-call nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the four-machine ordered cleanup closure is canonical artifact data"
    );
}

#[test]
fn three_call_nominal_cleanup_preserves_exact_source_order_through_codec_and_verifier() {
    let tokens = Lexer::new(THREE_CALL_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("three-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 5);
    let entry = &lowered.semantic_module.machines[0];
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [first, second, third] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target retains exactly three calls")
    };
    let callees = [first, second, third].map(|operation| {
        let OperationKind::CallUnit { callee, .. } = operation.kind else {
            panic!("cleanup helper is an ordinary Unit call")
        };
        callee
    });
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![entry.id, target.id, callees[0], callees[1], callees[2]]
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three-call nominal cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}
