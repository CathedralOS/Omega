//! Scalar-guarded Unit crash ceilings compose through exact evaluated operands.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

const SOURCE: &str = r#"
    machine identity(value: u16) -> u16 { value }
    boundary trait Sink { machine record(value: u16); }
    machine consume(value: u16) crashes Abort value == 0u16 {
        Sink::record(value);
    }
    data Main {}
    machine Main::main(selected: bool) crashes Abort {
        transition selected { true -> yes() _ -> no() }
        state yes() { consume(identity(0u16)); }
        state no() { consume(identity(7u16)); }
    }
"#;

#[test]
fn composed_unit_call_retains_a_scalar_guarded_crash_ceiling() {
    let checked = checked(SOURCE);
    let lowered = roundtrip(&checked);
    for selected in [false, true] {
        let (status, effects) = execute(&lowered, selected);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            effects,
            vec![vec![unsigned(if selected { 0 } else { 7 })]],
            "a true may-ceiling does not manufacture an executable crash"
        );
    }
}

fn roundtrip(checked: &checked_trees::CheckedTrees) -> lowered_psi::LoweredPsi {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("ordinary Unit callee retains its scalar-dependent crash ceiling");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("decoded guarded Unit call verifies independently");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    let published = terminal_production::produce_terminal_artifact(checked, "Main::main")
        .expect("guarded Unit closure publishes");
    assert_eq!(
        terminal_codec::decode_module(published.semantic_bytes()).unwrap(),
        module
    );
    lowered
}

fn runtime_source(equal: bool, transitive: bool, qualified: bool, left: u16, right: u16) -> String {
    let predicate = if equal {
        "left == right"
    } else {
        "left == 0u16 && right == 7u16"
    };
    let reversed = if equal {
        "right == left"
    } else {
        "right == 0u16 && left == 7u16"
    };
    let selected = if transitive { "relay" } else { "consume" };
    let source = format!(
        r#"
        machine identity(value: u16) -> u16
        requires 0u16 == 0u16
        ensures result == value
        {{ value }}
        machine maybe_crash(left: u16, right: u16) -> bool
        requires false == false
        ensures false == false
        crashes Abort {predicate}
        {{
            transition {predicate} {{ true -> fail() false -> false }}
            state fail() -> bool {{ crash Abort; }}
        }}
        boundary trait Sink {{
            machine before(left: u16, right: u16);
            machine after(value: bool);
        }}
        machine consume(left: u16, right: u16) crashes Abort {predicate} {{
            Sink::before(left, right);
            Sink::after(maybe_crash(left, right));
        }}
        machine relay(left: u16, right: u16) crashes Abort {reversed} {{
            consume(right, left);
        }}
        data Main {{}}
        machine Main::main(selected: bool) crashes Abort {{
            transition selected {{ true -> yes() _ -> no() }}
            state yes() {{ {selected}(identity(identity({left}u16)), identity({right}u16)); }}
            state no() {{ {selected}(identity({right}u16), identity(identity({left}u16))); }}
        }}
    "#
    );
    if qualified {
        source
            .replace("data Main {}", "data Main {} data Calls {}")
            .replace("consume(", "Calls::consume(")
            .replace("relay(", "Calls::relay(")
    } else {
        source
    }
}

#[test]
fn guarded_unit_calls_preserve_argument_order_transitive_effects_and_actual_crashes() {
    for equal in [false, true] {
        for transitive in [false, true] {
            for qualified in [false, true] {
                for (left, right) in [(0, 7), (7, 0), (7, 7)] {
                    let source = runtime_source(equal, transitive, qualified, left, right);
                    let lowered = roundtrip(&checked(&source));
                    for selected in [false, true] {
                        let (left, right) = if selected != transitive {
                            (left, right)
                        } else {
                            (right, left)
                        };
                        let crashes = if equal {
                            left == right
                        } else {
                            left == 0 && right == 7
                        };
                        let (status, effects) = execute(&lowered, selected);
                        let mut expected = vec![vec![unsigned(left), unsigned(right)]];
                        if crashes {
                            assert!(
                                matches!(status, TerminalExecutionStatus::Crashed(ref crash)
                                if crash.cause == terminal_psi::CrashCause::Abort),
                                "{source}: {status:?}"
                            );
                        } else {
                            assert_eq!(
                                status,
                                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                            );
                            expected.push(vec![TerminalScalarValue::Boolean(false)]);
                        }
                        assert_eq!(effects, expected, "{source}: selected={selected}");
                    }
                }
            }
        }
    }
}

#[test]
fn guarded_unit_calls_reject_missing_foreign_and_narrowed_terminal_routes() {
    let lowered = roundtrip(&checked(SOURCE));
    let module = &lowered.semantic_module;
    let (owner, block, operation, target) = module
        .machines
        .iter()
        .enumerate()
        .find_map(|(owner, machine)| {
            machine.blocks.iter().enumerate().find_map(|(block, body)| {
                body.operations
                    .iter()
                    .enumerate()
                    .find_map(|(operation, item)| match &item.kind {
                        terminal_psi::OperationKind::CallUnit {
                            callee,
                            crash_continuations,
                            ..
                        } if !crash_continuations.is_empty() => {
                            Some((owner, block, operation, *callee))
                        }
                        _ => None,
                    })
            })
        })
        .expect("guarded ordinary call retains a surviving continuation");
    let foreign_routes = module
        .machines
        .iter()
        .find(|machine| machine.id == target)
        .unwrap()
        .contract
        .crash_routes
        .clone();
    for mutation in 0..5 {
        let mut changed = module.clone();
        let terminal_psi::OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &mut changed.machines[owner].blocks[block].operations[operation].kind
        else {
            unreachable!();
        };
        match mutation {
            0 => crash_continuations.clear(),
            1 => *crash_continuations = foreign_routes.clone(),
            2 => changed.machines[owner].contract.crash_routes.clear(),
            3 | 4 => {
                let terminal_psi::CrashRouteGuard::Predicate(predicate) =
                    &crash_continuations[0].alternatives[0]
                else {
                    unreachable!();
                };
                let children = vec![predicate.proposition().clone()];
                let malformed = if mutation == 3 {
                    semantic_vocabulary::Proposition::Conjunction(children)
                } else {
                    semantic_vocabulary::Proposition::Disjunction(children)
                };
                crash_continuations[0].alternatives[0] = terminal_psi::CrashRouteGuard::Predicate(
                    terminal_psi::CrashPredicateTerm::new(malformed),
                );
            }
            _ => unreachable!(),
        }
        let operation = module.machines[owner].blocks[block].operations[operation].id;
        let expected = if mutation == 2 {
            terminal_verifier::ModuleError::CallCrashContinuationUncovered {
                operation,
                cause: terminal_psi::CrashCause::Abort,
            }
        } else {
            terminal_verifier::ModuleError::CallCrashContinuationsMismatch {
                operation,
                callee: target,
            }
        };
        assert_eq!(
            terminal_verifier::validate_module(&changed).unwrap_err(),
            expected,
            "route mutation={mutation} rejects during semantic validation, without a proof bundle"
        );
    }
}

#[test]
fn empty_attachment_receiver_keeps_both_boolean_crash_parameters_distinct() {
    for (name, position) in [("first", 0), ("second", 1)] {
        let source = format!(
            r#"
            boundary trait Sink {{ machine record(first: bool, second: bool); }}
            data Main {{}}
            machine Main::main(&self, first: bool, second: bool)
            crashes Abort {name}
            {{ Sink::record(first, second); }}
        "#
        );
        let lowered = roundtrip(&checked(&source));
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        assert_eq!(root.parameters.len(), 2);
        let [bucket] = root.contract.crash_routes.as_slice() else {
            panic!("one Abort ceiling");
        };
        assert_eq!(bucket.cause, terminal_psi::CrashCause::Abort);
        let [terminal_psi::CrashRouteGuard::Predicate(predicate)] = bucket.alternatives.as_slice()
        else {
            panic!("one exact Boolean parameter guard");
        };
        let semantic_vocabulary::Proposition::Equal(left, right) = predicate.proposition() else {
            panic!("Boolean parameter equality");
        };
        let expected = semantic_vocabulary::ScalarTerm::value(
            root.parameters[position].id,
            semantic_vocabulary::ScalarType::Boolean,
        );
        let other = semantic_vocabulary::ScalarTerm::value(
            root.parameters[1 - position].id,
            semantic_vocabulary::ScalarType::Boolean,
        );
        assert!([left, right].contains(&&expected));
        assert!(
            ![left, right].contains(&&other),
            "ambient self is not a scalar parameter slot"
        );
    }
}

#[test]
fn repeated_unit_arguments_collapse_equivalent_crash_connective_leaves() {
    for connective in ["&&", "||"] {
        let source = format!(
            r#"
            boundary trait Sink {{ machine record(left: u16, right: u16); }}
            machine consume(left: u16, right: u16)
            crashes Abort left == 0u16 {connective} right == 0u16
            {{ Sink::record(left, right); }}
            data Main {{}}
            machine Main::main(value: u16)
            crashes Abort value == 0u16 {connective} value == 0u16
            {{ consume(value, value); }}
            "#
        );
        let lowered = roundtrip(&checked(&source));
        let arguments = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallUnit { arguments, .. } => Some(arguments),
                _ => None,
            })
            .unwrap();
        assert_eq!(arguments.len(), 2);
        assert_eq!(arguments[0], arguments[1]);
    }
}

fn unsigned(value: u16) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            16,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(value)),
    }
}

#[derive(Default)]
struct Observe(Vec<Vec<TerminalScalarValue>>);

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("observable Sink effect");
        };
        self.0.push(arguments.clone());
        Ok(())
    }
}

fn execute(
    lowered: &lowered_psi::LoweredPsi,
    selected: bool,
) -> (TerminalExecutionStatus, Vec<Vec<TerminalScalarValue>>) {
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &evidence,
        &proof_admission::AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(selected)],
    )
    .unwrap();
    let mut observer = Observe::default();
    let status = execution
        .resume_with_effect_handler(
            &mut terminal_fuel::TerminalFuelMeter::unbounded(),
            &mut observer,
        )
        .unwrap();
    (status, observer.0)
}
