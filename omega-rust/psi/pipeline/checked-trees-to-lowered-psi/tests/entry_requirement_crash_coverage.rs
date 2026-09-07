//! Entry requirements justify crash routes without changing their entry namespace.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalInterpretError, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn with_caller(declarations: &str, call: &str) -> String {
    format!(
        r#"
        {declarations}
        machine value() -> bool
        requires true == true
        ensures true == true
        crashes Trap
        {{ {call} }}
        "#,
    )
}

fn assert_trap(source: &str) {
    assert_trap_with_module_check(source, |_| {});
}

fn assert_trap_with_module_check(
    source: &str,
    check_module: impl Fn(&terminal_psi::TerminalModule),
) {
    assert_trap_at_entry_with_module_check(source, "value", check_module);
}

fn assert_trap_at_entry_with_module_check(
    source: &str,
    entry: &str,
    check_module: impl Fn(&terminal_psi::TerminalModule),
) {
    assert_trap_with_entry_arguments(source, entry, false, check_module);
}

fn assert_trap_with_entry_arguments(
    source: &str,
    entry: &str,
    has_opaque_record_argument: bool,
    check_module: impl Fn(&terminal_psi::TerminalModule),
) {
    let artifact = {
        let checked = lower_typed_trees(typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
            .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        check_module(&lowered.semantic_module);
        (
            encode_module(&lowered.semantic_module).unwrap(),
            encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        )
    };
    let decoded_module = decode_module(&artifact.0).expect("decode terminal module");
    let decoded_proof = decode_proof_bundle(&artifact.1).expect("decode proof bundle");
    check_module(&decoded_module);
    terminal_verifier::verify_module(
        &decoded_module,
        &decoded_proof,
        &AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    // Boolean actuals remain source literals. The mixed fixture supplies only
    // an opaque record; its entry has no Requires and no field is observed.
    let result = if has_opaque_record_argument {
        let entry = decoded_module
            .machines
            .iter()
            .find(|machine| machine.id == decoded_module.entry)
            .expect("exact decoded entry");
        assert!(entry.parameters.is_empty(), "no host Boolean assumptions");
        assert!(entry.contract.requires.is_empty(), "no host entry proof");
        let [record] = entry.structural_parameters.as_slice() else {
            panic!("one exact opaque record argument");
        };
        struct NoEffects;
        impl terminal_interpreter::TerminalEffectHandler for NoEffects {
            fn handle_effect(
                &mut self,
                _effect: &terminal_interpreter::TerminalEffect,
            ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
                panic!("the trigger must trap before the Sink effect");
            }
        }
        terminal_interpreter::interpret_terminal_artifact_with_effect_handler_measured(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
            &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: record.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &mut NoEffects,
        )
        .map(|_| ())
    } else {
        interpret_terminal_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[])
            .map(|_| ())
    };
    assert!(
        matches!(result,
        Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
            if crash.cause == terminal_psi::CrashCause::Trap),
        "{source}"
    );
}

fn assert_unconditional_call_trap(source: &str) {
    assert_unconditional_call_trap_at_entry(source, "value");
}

fn assert_unconditional_call_trap_at_entry(source: &str, entry: &str) {
    assert_unconditional_call_trap_with_structural_arguments(source, entry, false);
}

fn assert_unconditional_call_trap_with_structural_arguments(
    source: &str,
    entry: &str,
    permits_structural_arguments: bool,
) {
    assert_trap_with_entry_arguments(source, entry, permits_structural_arguments, |module| {
        let mut checked_calls = 0;
        for operation in module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
        {
            let (callee, crash_continuations) = match &operation.kind {
                terminal_psi::OperationKind::Call {
                    callee,
                    crash_continuations,
                    ..
                } => (callee, crash_continuations),
                terminal_psi::OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    crash_continuations,
                    ..
                } => {
                    if !permits_structural_arguments {
                        assert!(structural_arguments.is_empty(), "scalar-only Unit fixture");
                        assert!(claim_transfers.is_empty(), "no structural claim transport");
                    }
                    (callee, crash_continuations)
                }
                _ => continue,
            };
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == *callee)
                .expect("exact retained call target");
            if !callee.parameters.is_empty() {
                continue;
            }
            assert_eq!(crash_continuations, &callee.contract.crash_routes);
            assert_eq!(
                crash_continuations,
                &[terminal_psi::CrashRouteBucket {
                    cause: terminal_psi::CrashCause::Trap,
                    alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                }]
            );
            checked_calls += 1;
        }
        assert_eq!(
            checked_calls, 1,
            "one unchanged unconditional trigger continuation"
        );
    });
}

#[test]
fn shared_record_entry_requirement_covers_unconditional_scalar_call() {
    let source = r#"
        data Flag { enabled: bool; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Sink::record(trigger()); }
        machine Main::value(record: &Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Helper::forward(record); }
    "#;
    assert_structural_entry_requirement_artifact(source);
}

fn assert_structural_entry_requirement_artifact(source: &str) {
    let checked = lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    for contract in &checked.facts.contract_plans.machines {
        for bucket in contract.crash.published() {
            for guard in bucket.alternative_guards() {
                if let checked_trees::CrashRouteGuard::Predicate(predicate) = guard {
                    assert!(
                        predicate.scalar_expression().is_some(),
                        "{source}: missing retained runtime predicate for {}",
                        checked.typed.symbols.name(contract.machine)
                    );
                }
            }
        }
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("encode shared entry module");
    let evidence = encode_proof_bundle(&lowered.proof_bundle).expect("encode shared entry proof");
    let module = decode_module(&semantics).expect("decode shared entry module");
    let proof = decode_proof_bundle(&evidence).expect("decode shared entry proof");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("shared entry hypotheses independently cover the unchanged call routes");
    let mut unconditional_calls = 0;
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let terminal_psi::OperationKind::Call {
            callee,
            crash_continuations,
            ..
        } = &operation.kind
        else {
            continue;
        };
        let target = module
            .machines
            .iter()
            .find(|machine| machine.id == *callee)
            .expect("retained call target");
        if target.parameters.is_empty() && target.structural_parameters.is_empty() {
            assert_eq!(crash_continuations, &target.contract.crash_routes);
            assert_eq!(
                crash_continuations,
                &[terminal_psi::CrashRouteBucket {
                    cause: terminal_psi::CrashCause::Trap,
                    alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                }]
            );
            unconditional_calls += 1;
        }
    }
    assert!(
        unconditional_calls > 0,
        "the unconditional callee is retained"
    );
    // This artifact still declares an entry requirement. Verification checks
    // call proof transport; no arbitrary host record is executed as its witness.
}

#[test]
fn shared_self_entry_requirement_covers_unconditional_scalar_call() {
    assert_structural_entry_requirement_artifact(
        r#"
        data Helper { enabled: bool; }
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(&self)
        requires self.enabled
        crashes Trap self.enabled
        { Sink::record(trigger()); }
        machine Main::value(record: &Helper)
        requires record.enabled
        crashes Trap record.enabled
        { record.forward(); }
        "#,
    );
}

#[test]
fn explicit_field_identities_preserve_structural_entry_hypotheses() {
    for identity in [0, 7] {
        let source = format!(
            "data Flag {{ #{identity} enabled: bool; #11 other: bool; }}\n\
             data Helper {{}}\ndata Main {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(record: &Flag)\nrequires record.enabled\ncrashes Trap record.enabled\n\
             {{ Sink::record(trigger()); }}\n\
             machine Main::value(record: &Flag)\nrequires record.enabled\ncrashes Trap record.enabled\n\
             {{ Helper::forward(record); }}",
        );
        assert_structural_entry_requirement_artifact(&source);
    }
}

#[test]
fn declared_structural_entry_requirement_survives_a_body_field_write() {
    assert_structural_entry_requirement_artifact(
        r#"
        data Flag { enabled: bool; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &mut Flag)
        requires record.enabled
        crashes Trap record.enabled
        { record.enabled = false; Sink::record(trigger()); }
        machine Main::value(record: &mut Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Helper::forward(record); }
        "#,
    );
}

#[test]
fn structural_requires_cannot_recover_corrupted_field_identity_from_spelling() {
    let source = r#"
        data Flag { enabled: bool; other: bool; }
        data Shadow { enabled: bool; }
        data Helper {}
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(record: &Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Sink::record(trigger()); }
        machine Main::value(record: &Flag)
        requires record.enabled
        crashes Trap record.enabled
        { Helper::forward(record); }
    "#;
    for corruption in ["foreign symbol", "missing symbol", "wrong spelling"] {
        let mut program = typed(source);
        let owner = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Shadow")
            .unwrap();
        let foreign = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "enabled" => {
                    Some(field.symbol)
                }
                _ => None,
            })
            .unwrap();
        let owner = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Flag")
            .unwrap();
        let wrong_name = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "other" => {
                    Some(field.name.clone())
                }
                _ => None,
            })
            .unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Helper::forward")
            .unwrap();
        let requirement = program
            .machine_contracts(machine)
            .iter()
            .find(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .unwrap();
        let expression = program
            .proof_facts
            .span_or_empty(requirement.facts)
            .iter()
            .find_map(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .unwrap();
        let mut pending = vec![expression];
        let mut selected = None;
        while let Some(expression) = pending.pop() {
            match program.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Member(_) => {
                    selected = Some(expression);
                    break;
                }
                typed_trees::expression::ExpressionNode::Binary(binary) => {
                    pending.extend([binary.right, binary.left])
                }
                typed_trees::expression::ExpressionNode::Unary(unary) => {
                    pending.push(unary.operand)
                }
                _ => {}
            }
        }
        let typed_trees::expression::ExpressionNode::Member(member) = program
            .expression_table
            .expression_mut(selected.expect("retained Requires member"))
        else {
            unreachable!();
        };
        assert!(member.member_symbol.is_valid());
        assert!(foreign.is_valid());
        assert_ne!(member.member_symbol, foreign);
        match corruption {
            "foreign symbol" => member.member_symbol = foreign,
            "missing symbol" => member.member_symbol = Default::default(),
            "wrong spelling" => member.member = wrong_name,
            _ => unreachable!(),
        }
        let diagnostics = match lower_typed_trees(program) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("corrupted Requires identity was accepted: {corruption}"),
        };
        assert!(!diagnostics.is_empty(), "{corruption}");
    }
}

#[test]
fn structural_entry_formulas_preserve_owned_shared_and_mutable_borrow_snapshots() {
    for ownership in ["", "&", "&mut "] {
        for (record_type, predicate) in [
            ("Flag", "record.enabled"),
            ("Flag", "!record.enabled"),
            ("Flag", "record.enabled && record.other"),
            ("Flag", "record.enabled == record.other"),
            ("Envelope", "record.inner.enabled"),
            ("Envelope", "record.inner.enabled == record.inner.other"),
        ] {
            let source = format!(
                "data Flag {{ enabled: bool; other: bool; }}\n\
                 data Envelope {{ inner: Flag; }}\ndata Helper {{}}\ndata Main {{}}\n\
                 boundary trait Sink {{ machine record(value: bool); }}\n\
                 machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine Helper::forward(record: {ownership}{record_type})\n\
                 requires {predicate}\ncrashes Trap {predicate}\n{{ Sink::record(trigger()); }}\n\
                 machine Main::value(record: {ownership}{record_type})\n\
                 requires {predicate}\ncrashes Trap {predicate}\n{{ Helper::forward(record); }}",
            );
            assert_structural_entry_requirement_artifact(&source);
        }
    }
}

#[test]
fn structural_entry_requirement_does_not_authorize_a_different_field_or_root() {
    for route in ["record.other", "other.enabled", "other.other"] {
        let source = format!(
            "data Flag {{ enabled: bool; other: bool; }}\ndata Helper {{}}\ndata Main {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(record: &Flag, other: &Flag)\n\
             requires record.enabled\ncrashes Trap {route}\n{{ Sink::record(trigger()); }}\n\
             machine Main::value(record: &Flag, other: &Flag)\n\
             requires record.enabled\ncrashes Trap\n{{ Helper::forward(record, other); }}",
        );
        let diagnostics = match lower_typed_trees(typed(&source)) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("a different structural entry identity must reject: {source}"),
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("Helper::forward")
                    && diagnostic.message.contains("uncovered Trap crash route")
            }),
            "the exact field/root crash coverage check must reject: {source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn attached_boolean_entry_requirement_covers_unconditional_scalar_call() {
    assert_unconditional_call_trap_at_entry(
        r#"
        data Main {}
        data Helper {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(flag: bool)
        requires flag
        crashes Trap flag
        { Sink::record(trigger()); }
        machine Main::value()
        crashes Trap
        { Helper::forward(true); }
        "#,
        "Main::value",
    );
}

#[test]
fn attached_boolean_entry_formulas_keep_original_scalar_parameters() {
    for (predicate, arguments) in [
        ("left && right", "true, true"),
        ("!right", "true, false"),
        ("left == right", "false, false"),
        ("left == right", "true, true"),
        ("!(left && right)", "false, true"),
    ] {
        let source = format!(
            "data Main {{}}\ndata Helper {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(left: bool, right: bool)\nrequires {predicate}\ncrashes Trap {predicate}\n\
             {{ Sink::record(trigger()); }}\n\
             machine Main::value()\ncrashes Trap\n{{ Helper::forward({arguments}); }}",
        );
        assert_unconditional_call_trap_at_entry(&source, "Main::value");
    }
}

#[test]
fn attached_entry_requirements_cannot_substitute_another_boolean_formal() {
    for (requirement, route, arguments) in [
        ("left", "right", "true, false"),
        ("!left", "!right", "false, true"),
    ] {
        let source = format!(
            "data Main {{}}\ndata Helper {{}}\n\
             boundary trait Sink {{ machine record(value: bool); }}\n\
             machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine Helper::forward(left: bool, right: bool)\nrequires {requirement}\ncrashes Trap {route}\n\
             {{ Sink::record(trigger()); }}\n\
             machine Main::value()\ncrashes Trap\n{{ Helper::forward({arguments}); }}",
        );
        let diagnostics = match lower_typed_trees(typed(&source)) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("distinct attached formals cannot authorize each other: {source}"),
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("Helper::forward")
                    && diagnostic.message.contains("uncovered Trap crash route")
            }),
            "the exact call crash coverage check must reject: {source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn mixed_attached_entry_requirement_uses_the_original_nonfirst_boolean() {
    let source = r#"
        data Main {}
        data Helper {}
        data Box { value: u32; }
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine Helper::forward(left: bool, record: Box, right: bool)
        requires right
        crashes Trap right
        { Sink::record(trigger()); }
        machine Main::value(record: Box)
        crashes Trap
        { Helper::forward(false, record, true); }
    "#;
    let checked = lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let missing = ["Helper::forward", "Main::value"]
        .into_iter()
        .filter(|name| {
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == *name)
                .expect("exact authored mixed fixture machine");
            !checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .any(|plan| plan.machine == machine.symbol)
        })
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "mixed fixture lacks checked Unit plans for {missing:?}"
    );
    assert_unconditional_call_trap_with_structural_arguments(source, "Main::value", true);
}

#[test]
fn compound_boolean_equality_entry_requirement_covers_unconditional_unit_call() {
    let declarations = r#"
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine forward(a: bool, b: bool, c: bool, d: bool)
        requires (a && b) == (c || d)
        crashes Trap (a && b) == (c || d)
        { Sink::record(trigger()); }
        machine Main::value()
        crashes Trap
        { forward(true, true, false, true); }
    "#;
    assert_unconditional_call_trap_at_entry(declarations, "Main::value");
}

#[test]
fn atomic_boolean_entry_requirement_covers_unconditional_unit_call() {
    assert_unconditional_call_trap_at_entry(
        r#"
        data Main {}
        boundary trait Sink { machine record(value: bool); }
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine forward(a: bool)
        requires a
        crashes Trap a
        { Sink::record(trigger()); }
        machine Main::value()
        crashes Trap
        { forward(true); }
        "#,
        "Main::value",
    );
}

#[test]
fn compound_boolean_equality_entry_requirement_covers_unconditional_call() {
    let declarations = r#"
        machine trigger() -> bool
        crashes Trap
        { crash Trap; }
        machine forward(a: bool, b: bool, c: bool, d: bool) -> bool
        requires (a && b) == (c || d)
        crashes Trap (a && b) == (c || d)
        { trigger() }
    "#;
    assert_unconditional_call_trap(&with_caller(
        declarations,
        "forward(true, true, false, true)",
    ));
}

#[test]
fn compound_boolean_entry_routes_follow_equality_polarities_and_entry_snapshots() {
    for bits in 0u8..16 {
        let [a, b, c, d] = [0, 1, 2, 3].map(|position| bits & (1 << position) != 0);
        let equal = (a && b) == (c || d);
        for (predicate, requires_equal) in [
            ("(a && b) == (c || d)", true),
            ("(a && b) != (c || d)", false),
            ("!((a && b) == (c || d))", false),
            ("!((a && b) != (c || d))", true),
        ] {
            if equal != requires_equal {
                continue;
            }
            for mutable in [false, true] {
                let parameters = if mutable {
                    "mut a: bool, mut b: bool, mut c: bool, mut d: bool"
                } else {
                    "a: bool, b: bool, c: bool, d: bool"
                };
                let body = match (mutable, requires_equal) {
                    (false, _) => "trigger()",
                    // Each write sequence falsifies the current predicate;
                    // the published route still describes the entry operands.
                    (true, true) => "a = true; b = true; c = false; d = false; trigger()",
                    (true, false) => "a = false; b = false; c = false; d = false; trigger()",
                };
                let declarations = format!(
                    "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                     machine forward({parameters}) -> bool\nrequires {predicate}\ncrashes Trap {predicate}\n{{ {body} }}",
                );
                assert_unconditional_call_trap(&with_caller(
                    &declarations,
                    &format!("forward({a}, {b}, {c}, {d})"),
                ));
            }
        }
    }
}

#[test]
fn nested_compound_boolean_equalities_retain_each_operand_polarity() {
    let predicate = "((a && b) == (c || d)) == ((a || c) == (b && d))";
    for bits in 0u8..16 {
        let [a, b, c, d] = [0, 1, 2, 3].map(|position| bits & (1 << position) != 0);
        if ((a && b) == (c || d)) != ((a || c) == (b && d)) {
            continue;
        }
        for (parameters, body) in [
            ("a: bool, b: bool, c: bool, d: bool", "trigger()"),
            (
                "mut a: bool, mut b: bool, mut c: bool, mut d: bool",
                "a = false; b = false; c = false; d = true; trigger()",
            ),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {predicate}\ncrashes Trap {predicate}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(
                &declarations,
                &format!("forward({a}, {b}, {c}, {d})"),
            ));
        }
    }
}

#[test]
fn compound_boolean_entry_requirements_do_not_authorize_opposite_or_missing_routes() {
    for (requirement, route, arguments, body) in [
        (
            "(a && b) == (c || d)",
            "(a && b) != (c || d)",
            "false, false, false, false",
            "a = true; b = true; c = false; d = false; trigger()",
        ),
        (
            "(a && b) != (c || d)",
            "(a && b) == (c || d)",
            "true, true, false, false",
            "a = false; b = false; c = false; d = false; trigger()",
        ),
        (
            "(a && b) == (c || d)",
            "a",
            "false, false, false, false",
            "a = true; b = true; c = true; d = true; trigger()",
        ),
    ] {
        for mutable in [false, true] {
            let parameters = if mutable {
                "mut a: bool, mut b: bool, mut c: bool, mut d: bool"
            } else {
                "a: bool, b: bool, c: bool, d: bool"
            };
            let body = if mutable { body } else { "trigger()" };
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            let source = with_caller(&declarations, &format!("forward({arguments})"));
            let diagnostics = match lower_typed_trees(typed(&source)) {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("an unproved compound crash route must reject: {source}"),
            };
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("call from `forward` to `trigger`")
                        && diagnostic.message.contains("uncovered Trap crash route")
                }),
                "the exact call crash coverage check must reject: {source}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn common_disjunctive_entry_consequence_covers_unconditional_call() {
    for (parameters, body) in [
        ("a: bool, b: bool, c: bool", "trigger()"),
        (
            "mut a: bool, mut b: bool, mut c: bool",
            "a = false; b = false; c = false; trigger()",
        ),
    ] {
        for arguments in ["true, true, false", "true, false, true", "true, true, true"] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires (a && b) || (a && c)\ncrashes Trap a\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(
                &declarations,
                &format!("forward({arguments})"),
            ));
        }
    }
}

#[test]
fn nested_disjunctive_entry_consequences_preserve_complete_published_predicates() {
    for (requirement, route, arguments) in [
        (
            "(a && b) || ((a && c) || (a && d))",
            "a",
            "true, false, false, true",
        ),
        (
            "(a && b) || ((a && c) || (a && d))",
            "a",
            "true, true, false, false",
        ),
        (
            "((a && b) || (a && c)) && ((d && b) || (d && c))",
            "a && d",
            "true, true, false, true",
        ),
        (
            "((a && b) || (a && c)) && ((d && b) || (d && c))",
            "a && d",
            "true, false, true, true",
        ),
    ] {
        for (parameters, body) in [
            ("a: bool, b: bool, c: bool, d: bool", "trigger()"),
            (
                "mut a: bool, mut b: bool, mut c: bool, mut d: bool",
                "a = false; b = false; c = false; d = false; trigger()",
            ),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(
                &declarations,
                &format!("forward({arguments})"),
            ));
        }
    }
}

#[test]
fn a_missing_disjunctive_entry_consequence_cannot_be_repaired_by_body_writes() {
    for (requirement, route, arguments) in [
        ("(a && b) || (b && c)", "a", "false, true, true"),
        (
            "(a && b) || ((a && c) || (b && c))",
            "a",
            "false, true, true",
        ),
        ("(a && b) || (a && c)", "a && b", "true, false, true"),
    ] {
        for (parameters, body) in [
            ("a: bool, b: bool, c: bool", "trigger()"),
            (
                "mut a: bool, mut b: bool, mut c: bool",
                "a = true; b = true; c = true; trigger()",
            ),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            let source = with_caller(&declarations, &format!("forward({arguments})"));
            let diagnostics = match lower_typed_trees(typed(&source)) {
                Err(diagnostics) => diagnostics,
                Ok(_) => panic!("a missing entry consequence must reject: {source}"),
            };
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("call from `forward` to `trigger`")
                        && diagnostic.message.contains("uncovered Trap crash route")
                }),
                "the exact call crash coverage check must reject: {source}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn negated_entry_requirement_covers_unconditional_call_after_mutable_reassignment() {
    for (parameters, body) in [
        ("flag: bool", "trigger()"),
        ("mut flag: bool", "flag = true; trigger()"),
    ] {
        let declarations = format!(
            "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine forward({parameters}) -> bool\nrequires !flag\ncrashes Trap !flag\n{{ {body} }}",
        );
        assert_unconditional_call_trap(&with_caller(&declarations, "forward(false)"));
    }
}

#[test]
fn false_equality_entry_requirements_cover_equivalent_negated_call_routes() {
    for (requirement, route) in [
        ("flag == false", "flag == false"),
        ("flag == false", "!flag"),
        ("!flag", "flag == false"),
    ] {
        for (parameters, body) in [
            ("flag: bool", "trigger()"),
            ("mut flag: bool", "flag = true; trigger()"),
        ] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(&declarations, "forward(false)"));
        }
    }
}

#[test]
fn negated_conjunction_entry_call_coverage_keeps_original_mutable_operands() {
    for (parameters, body) in [
        ("flag: bool, other: bool", "trigger()"),
        (
            "mut flag: bool, mut other: bool",
            "flag = true; other = true; trigger()",
        ),
    ] {
        for route in ["!(flag && other)", "(flag && other) == false"] {
            for arguments in ["false, true", "true, false", "false, false"] {
                let declarations = format!(
                    "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                     machine forward({parameters}) -> bool\nrequires !(flag && other)\ncrashes Trap {route}\n{{ {body} }}",
                );
                assert_unconditional_call_trap(&with_caller(
                    &declarations,
                    &format!("forward({arguments})"),
                ));
            }
        }
    }
}

#[test]
fn negated_disjunction_entry_call_coverage_keeps_original_mutable_operands() {
    for (parameters, body) in [
        ("flag: bool, other: bool", "trigger()"),
        (
            "mut flag: bool, mut other: bool",
            "flag = true; other = true; trigger()",
        ),
    ] {
        for route in ["!(flag || other)", "false == (flag || other)"] {
            let declarations = format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine forward({parameters}) -> bool\nrequires !(flag || other)\ncrashes Trap {route}\n{{ {body} }}",
            );
            assert_unconditional_call_trap(&with_caller(&declarations, "forward(false, false)"));
        }
    }
}

#[test]
fn all_crash_scalar_callee_retains_its_entry_requirement() {
    assert_trap(&with_caller(
        "machine trigger(flag: bool) -> bool\nrequires flag\ncrashes Trap\n{ crash Trap; }",
        "trigger(true)",
    ));
}

#[test]
fn proved_entry_requirement_covers_unconditional_crash_and_survives_a_body_write() {
    for (mutability, body) in [("", "crash Trap;"), ("mut ", "flag = false; crash Trap;")] {
        let declarations = format!(
            r#"
            machine trigger({mutability}flag: bool) -> bool
            requires flag
            crashes Trap flag
            {{ {body} }}
            "#,
        );
        assert_trap(&with_caller(&declarations, "trigger(true)"));
    }
}

#[test]
fn entry_requirements_cover_an_unconditional_callee_without_rewriting_its_routes() {
    for (parameters, requirement, body, arguments) in [
        ("flag: bool", "flag", "trigger()", "true"),
        ("mut flag: bool", "flag", "flag = false; trigger()", "true"),
        (
            "flag: bool, other: bool",
            "flag && other",
            "trigger()",
            "true, true",
        ),
    ] {
        let declarations = format!(
            "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine forward({parameters}) -> bool\nrequires {requirement}\ncrashes Trap flag\n{{ {body} }}",
        );
        assert_trap(&with_caller(
            &declarations,
            &format!("forward({arguments})"),
        ));
    }
}

#[test]
fn entry_requirement_also_covers_propagated_scalar_call_routes() {
    let declarations = r#"
        machine trigger(flag: bool) -> bool
        requires flag
        crashes Trap flag
        { crash Trap; }
        machine forward(flag: bool) -> bool
        requires flag
        crashes Trap flag
        { trigger(flag) }
    "#;
    assert_trap(&with_caller(declarations, "forward(true)"));
}

#[test]
fn named_state_parameters_do_not_rebind_the_machine_entry_crash_condition() {
    for parameter in ["flag", "different"] {
        let declarations = format!(
            r#"
            machine trigger(flag: bool) -> bool
            requires flag
            crashes Trap flag
            {{
                transition {{ _ -> finish(false) }}
                state finish({parameter}: bool) -> bool {{ crash Trap; }}
            }}
            "#,
        );
        assert_trap(&with_caller(&declarations, "trigger(true)"));
    }
}

#[test]
fn changing_a_false_entry_parameter_cannot_authorize_its_entry_guarded_crash() {
    let source = with_caller(
        r#"
        machine trigger(mut flag: bool) -> bool
        requires !flag
        crashes Trap flag
        { flag = true; crash Trap; }
        "#,
        "trigger(false)",
    );
    let diagnostics = lower_typed_trees(typed(&source))
        .expect_err("the final true value cannot stand in for the false entry crash guard");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")
                && diagnostic.message.contains("crash")),
        "the actual crash coverage check must reject: {diagnostics:#?}"
    );
}
