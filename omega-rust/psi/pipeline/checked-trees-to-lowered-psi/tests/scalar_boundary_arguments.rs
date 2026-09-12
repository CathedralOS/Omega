use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_psi::OperationKind;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait Console {
        machine exit_process(return_code: i32)
        reaches Console;
    }

    data Root {}
    machine Root::enter()
    reaches Console
    {
        Console::exit_process(37);
    }
"#;

#[test]
fn guarded_boundary_crash_contract_survives_source_lowering() {
    let source = r#"
        boundary trait Sink {
            machine record(value: u16) crashes Abort value == 0u16;
        }
        data Root {}
        machine Root::enter(value: u16) reaches Sink crashes Abort value == 0u16 {
            Sink::record(value);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("guarded boundary source should lower");
    let routes = &lowered.semantic_module.boundary_machines[0].crash_routes;
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].cause, terminal_psi::CrashCause::Abort);
    assert!(matches!(
        routes[0].alternatives.as_slice(),
        [terminal_psi::CrashRouteGuard::Predicate(_)]
    ));
    verify_roundtrip(&lowered);
    exercise_boundary_outcomes(&lowered, &[unsigned16(0)], true);
    exercise_boundary_outcomes(&lowered, &[unsigned16(1)], false);
}

struct BoundaryOutcome {
    result: terminal_interpreter::TerminalEffectResult,
    calls: usize,
}

impl terminal_interpreter::TerminalEffectHandler for BoundaryOutcome {
    fn handle_effect(
        &mut self,
        _: &terminal_interpreter::TerminalEffect,
    ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
        self.calls += 1;
        Ok(())
    }

    fn handle_effect_result(
        &mut self,
        effect: &terminal_interpreter::TerminalEffect,
    ) -> Result<
        terminal_interpreter::TerminalEffectResult,
        terminal_interpreter::TerminalEffectRejection,
    > {
        self.handle_effect(effect)?;
        Ok(self.result.clone())
    }
}

fn unsigned16(value: u128) -> terminal_interpreter::TerminalScalarValue {
    terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn exercise_boundary_outcomes(
    lowered: &lowered_psi::LoweredPsi,
    arguments: &[terminal_interpreter::TerminalScalarValue],
    abort_permitted: bool,
) {
    use terminal_interpreter::{
        TerminalCrashSite, TerminalEffectResult, TerminalExecution, TerminalExecutionResult,
        TerminalExecutionStatus, TerminalInterpretError, TerminalStructuralValue,
    };
    use terminal_psi::CrashCause;
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let structural = root
        .structural_parameters
        .iter()
        .map(|parameter| TerminalStructuralValue {
            opaque_identity: 701,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let normal = match root.result {
        terminal_psi::TerminalMachineResult::Unit => TerminalEffectResult::Unit,
        terminal_psi::TerminalMachineResult::Scalar(_) => {
            TerminalEffectResult::Scalar(terminal_interpreter::TerminalScalarValue::Boolean(true))
        }
        _ => panic!("test expects Unit or Boolean result"),
    };
    for returned in [
        normal,
        TerminalEffectResult::Crash(CrashCause::Abort),
        TerminalEffectResult::Crash(CrashCause::Trap),
    ] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            arguments,
            &structural,
        )
        .expect("crash-capable boundary module starts");
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        let mut handler = BoundaryOutcome {
            result: returned.clone(),
            calls: 0,
        };
        let mut empty_meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
        assert!(matches!(
            execution
                .resume_with_effect_handler(&mut empty_meter, &mut handler)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(handler.calls, 0);
        let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
        let outcome = execution.resume_with_effect_handler(&mut meter, &mut handler);
        assert_eq!(handler.calls, 1);
        match returned {
            TerminalEffectResult::Unit | TerminalEffectResult::Scalar(_) => {
                let expected = match returned {
                    TerminalEffectResult::Scalar(value) => TerminalExecutionResult::Scalar(value),
                    _ => TerminalExecutionResult::Unit,
                };
                assert_eq!(
                    outcome.unwrap(),
                    TerminalExecutionStatus::Complete(expected)
                );
                assert_eq!(execution.live_claim_frontier().count(), 0);
            }
            TerminalEffectResult::Crash(CrashCause::Abort) if abort_permitted => {
                let outcome = outcome.unwrap();
                let TerminalExecutionStatus::Crashed(crash) = &outcome else {
                    panic!("{outcome:?}")
                };
                let [effect] = execution.effects() else {
                    panic!("one invoked boundary")
                };
                let terminal_interpreter::TerminalEffect::BoundaryCall {
                    operation,
                    boundary,
                    ..
                } = effect
                else {
                    panic!("boundary invocation")
                };
                assert_eq!(
                    crash.site,
                    TerminalCrashSite::BoundaryCall {
                        machine: root.id,
                        block: root.entry,
                        operation: *operation,
                        boundary: *boundary,
                    }
                );
                assert_eq!(crash.frontier_lower_bound, claims);
                assert_eq!(execution.live_claim_frontier().collect::<Vec<_>>(), claims);
                let usage = meter.usage().total_units();
                assert_eq!(
                    execution
                        .resume_with_effect_handler(&mut meter, &mut handler)
                        .unwrap(),
                    outcome
                );
                assert_eq!(meter.usage().total_units(), usage);
                assert_eq!(handler.calls, 1);
            }
            TerminalEffectResult::Crash(_) => {
                assert!(matches!(
                    outcome,
                    Err(TerminalInterpretError::BoundaryCrashNotPermitted { .. })
                ));
                assert_eq!(execution.live_claim_frontier().collect::<Vec<_>>(), claims);
                assert!(execution.effects().is_empty());
            }
            _ => unreachable!(),
        }
    }
}

fn verify_roundtrip(lowered: &lowered_psi::LoweredPsi) {
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("encode module");
    let evidence =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proofs");
    let module = terminal_codec::decode_module(&semantic).expect("decode module");
    let proof = terminal_codec::decode_proof_bundle(&evidence).expect("decode proofs");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("source-free verifier replays boundary crash substitution");
}

#[test]
fn mixed_boundary_signature_uses_dense_scalar_crash_formals() {
    let source = r#"
        data Token {}
        boundary trait Sink {
            machine record(token: Token, selected: bool, value: u16)
            crashes Abort selected && value == 0u16;
        }
        data Root {}
        machine Root::enter(selected: bool, token: Token, value: u16)
        reaches Sink
        crashes Abort selected && value == 0u16 {
            Sink::record(token, selected, value);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("mixed boundary crash source should lower");
    assert_eq!(
        lowered.semantic_module.boundary_machines[0]
            .scalar_parameters
            .len(),
        2
    );
    assert_eq!(
        lowered.semantic_module.boundary_machines[0]
            .crash_routes
            .len(),
        1
    );
    verify_roundtrip(&lowered);
    for (selected, value, permitted) in [(true, 0, true), (false, 0, false), (true, 1, false)] {
        exercise_boundary_outcomes(
            &lowered,
            &[
                terminal_interpreter::TerminalScalarValue::Boolean(selected),
                unsigned16(value),
            ],
            permitted,
        );
    }
}

#[test]
fn boundary_crash_callers_cannot_omit_or_swap_the_published_cause() {
    for ceiling in ["", "crashes Trap", "crashes Abort value != 0u16"] {
        let source = format!(
            r#"
            boundary trait Sink {{ machine record(value: u16) crashes Abort value == 0u16; }}
            pub machine enter(value: u16) invokes Sink; {ceiling} {{ Sink::record(value); }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let errors = lower_typed_trees(typed).expect_err("published caller must cover Abort");
        assert!(
            errors.iter().any(|error| error.message.contains("Abort")),
            "{errors:?}"
        );
    }
}

#[test]
fn literal_false_boundary_route_has_no_surviving_cause() {
    let source = r#"
        boundary trait Sink { machine record() crashes Abort false; }
        data Root {}
        machine Root::enter() reaches Sink { Sink::record(); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("false boundary route should normalize before lowering");
    assert!(
        lowered.semantic_module.boundary_machines[0]
            .crash_routes
            .is_empty()
    );
    verify_roundtrip(&lowered);
}

#[test]
fn guarded_boundary_contracts_survive_attached_and_scalar_result_producers() {
    for source in [
        r#"
            pub data Sink {}
            boundary machine Sink::record(value: u16) crashes Abort value == 0u16;
            data Root {}
            machine Root::enter(value: u16) crashes Abort value == 0u16 {
                Sink::record(value);
            }
        "#,
        r#"
            boundary trait Sink { machine record(value: u16) -> bool crashes Abort value == 0u16; }
            data Root {}
            machine Root::enter(value: u16) -> bool reaches Sink crashes Abort value == 0u16 {
                let result: bool = Sink::record(value);
                result
            }
        "#,
        r#"
            boundary trait Sink { machine record(value: u16) -> bool crashes Abort value == 0u16; }
            data Root {}
            machine Root::enter(value: u16) -> bool reaches Sink crashes Abort value == 0u16 {
                Sink::record(value)
            }
        "#,
        r#"
            boundary trait Sink { machine record(first: u16, second: u16) crashes Abort first == 0u16 && second == 7u16; }
            data Root {}
            machine Root::enter(left: u16, right: u16) reaches Sink crashes Abort right == 0u16 && left == 7u16 {
                Sink::record(right, left);
            }
        "#,
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert_eq!(
            lowered.semantic_module.boundary_machines[0]
                .crash_routes
                .len(),
            1
        );
        verify_roundtrip(&lowered);
        if matches!(
            lowered.semantic_module.machines[0].result,
            terminal_psi::TerminalMachineResult::Scalar(_)
        ) {
            exercise_boundary_outcomes(&lowered, &[unsigned16(0)], true);
            exercise_boundary_outcomes(&lowered, &[unsigned16(1)], false);
        }
    }
}

#[test]
fn structural_boundary_crash_guards_reject_without_losing_the_contract() {
    let source = r#"
        data Flag { enabled: bool; }
        boundary trait Sink { machine record(flag: Flag) crashes Abort flag.enabled; }
        data Root {}
        machine Root::enter(flag: Flag) reaches Sink crashes Abort { Sink::record(flag); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("source contract checks");
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect_err("unsupported structural boundary crash contract must reject");
    assert!(format!("{error:?}").contains("guarded crash route"));
}

#[test]
fn checked_source_preserves_exact_scalar_boundary_argument_into_terminal_psi() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("scalar boundary source should lower");

    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("one boundary declaration should be retained")
    };
    assert_eq!(
        boundary.scalar_parameters,
        [ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).expect("i32")
        )]
    );
    assert!(boundary.structural_parameters.is_empty());

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [literal, call] = entry.blocks[0].operations.as_slice() else {
        panic!("literal materialization must precede the boundary call")
    };
    assert!(matches!(
        literal.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Signed(37)
        }
    ));
    let OperationKind::BoundaryCall {
        boundary: called,
        arguments,
        structural_arguments,
        ..
    } = &call.kind
    else {
        panic!("second operation should be the boundary call")
    };
    assert_eq!(*called, boundary.id);
    assert_eq!(
        arguments,
        &[literal.result.scalar().expect("literal result").id]
    );
    assert!(structural_arguments.is_empty());

    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("scalar boundary module should encode canonically");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical scalar boundary bytes"),
        lowered.semantic_module
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verification accepts the source-produced scalar boundary call");
}
