use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    interpret_terminal_artifact_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    boundary trait Output { machine byte(value: u8) reaches Output; }
    data Writer {}
    machine Writer::quiet() {}
    machine Writer::newline(enabled: bool) reaches Output {
        transition enabled {
            true -> emit()
            false -> skip()
        }
        state emit() { Output::byte(10); }
        state skip() { Writer::quiet(); }
    }
    data Root {}
    machine Root::enter() reaches Output {
        Writer::newline(true);
        Writer::newline(false);
        Output::byte(99);
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

#[test]
fn ordinary_calls_retain_multistate_branches_and_return_to_caller() {
    let checked = checked(SOURCE);
    assert_eq!(
        checked.facts.flow.terminal_unit_effects.composed_machines[0]
            .attachment_type_identity
            .as_deref(),
        Some("named(name(Writer))")
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("ordinary caller retains its multistate helper");
    assert_eq!(lowered.semantic_module.machines.len(), 3);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .filter(|machine| machine.blocks.len() == 3)
            .count(),
        1
    );
    assert_eq!(execute(&lowered), [10, 99]);
    assert_optimized(lowered, &[10, 99]);
}

fn execute(lowered: &lowered_psi::LoweredPsi) -> Vec<u128> {
    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("byte output");
            };
            let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
                panic!("one byte");
            };
            let semantic_vocabulary::IntegerValue::Unsigned(value) = value else {
                panic!("unsigned byte");
            };
            *value
        })
        .collect()
}

fn assert_optimized(lowered: lowered_psi::LoweredPsi, expected: &[u128]) {
    let selections = optimization::PsiOptimizationSelections::new([
        optimization::PsiOptimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let optimized = lowered_psi_to_lowered_psi::run_psi_optimization(lowered, selections).unwrap();
    assert_eq!(execute(optimized.lowered()), expected);
}

#[test]
fn nested_composed_and_ordinary_calls_share_bodies_and_continue_in_order() {
    let source = SOURCE
        .replace(
            "data Root {}",
            r#"
        machine Writer::choose(enabled: bool) reaches Output {
            transition enabled { true -> emit() false -> skip() }
            state emit() { Writer::newline(true); }
            state skip() { Writer::newline(false); }
        }
        machine Writer::relay(enabled: bool) reaches Output { Writer::choose(enabled); }
        data Root {}
    "#,
        )
        .replace(
            "Writer::newline(true);\n        Writer::newline(false);",
            "Writer::relay(true);\n        Writer::relay(false);",
        );
    let checked = checked(&source);
    for (root, arguments, expected) in [
        ("Root::enter", None, vec![10, 99]),
        ("Writer::choose", Some(true), vec![10]),
        ("Writer::choose", Some(false), vec![]),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, root).unwrap();
        if let Some(enabled) = arguments {
            let execution = interpret_terminal_artifact_measured(
                &encode_module(&lowered.semantic_module).unwrap(),
                &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
                &AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(enabled)],
            )
            .unwrap();
            assert_eq!(execution.value(), TerminalExecutionResult::Unit);
            assert_eq!(execution.effects().len(), expected.len());
        } else {
            assert_eq!(
                lowered.semantic_module.machines.len(),
                5,
                "each authored body appears once"
            );
            assert_eq!(
                lowered
                    .semantic_module
                    .machines
                    .iter()
                    .filter(|machine| machine.blocks.len() == 3)
                    .count(),
                2
            );
            assert_eq!(execute(&lowered), expected);
            assert_optimized(lowered, &expected);
        }
    }
}

#[test]
fn composed_leaf_scalar_calls_share_ids_and_proof_obligations_with_caller() {
    let source = SOURCE
        .replace(
            "data Writer {}",
            r#"
        data Scalar {}
        machine Scalar::identity(value: u8) -> u8
        requires 0u8 == 0u8
        ensures result == value
        { value }
        data Writer {}
    "#,
        )
        .replace("Output::byte(10)", "Output::byte(Scalar::identity(10))")
        .replace(
            "data Root {}",
            r#"
            machine Writer::other(enabled: bool) reaches Output {
                transition enabled { true -> emit() false -> skip() }
                state emit() { Output::byte(Scalar::identity(11)); }
                state skip() { Writer::quiet(); }
            }
            data Root {}
        "#,
        )
        .replace(
            "Writer::newline(false);",
            "Writer::newline(false); Writer::other(true);",
        )
        .replace("Output::byte(99)", "Output::byte(Scalar::identity(99))");
    let checked = checked(&source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
    assert_eq!(lowered.semantic_module.machines.len(), 5);
    assert_eq!(execute(&lowered), [10, 11, 99]);
    assert_optimized(lowered, &[10, 11, 99]);
}

#[test]
fn missing_or_duplicate_composed_callee_is_rejected() {
    for mutation in 0..3 {
        let mut checked = checked(SOURCE);
        let plans = &mut checked.facts.flow.terminal_unit_effects;
        let callee = plans.composed_machines[0].clone();
        if mutation == 1 {
            let mut impostor = plans.machines[0].clone();
            impostor.machine = callee.machine;
            impostor.state = callee.states[0].state;
            plans.machines.push(impostor);
        } else if mutation == 2 {
            plans.composed_machines.push(callee);
        } else {
            plans.composed_machines.clear();
        }
        let error =
            checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap_err();
        let expected = match mutation {
            0 => "missing a checked transitive machine plan",
            1 => "direct Unit scalar parameters do not rejoin the exact typed source partition",
            2 => "duplicate checked machine plans",
            _ => unreachable!(),
        };
        assert!(format!("{error:?}").contains(expected), "{error:?}");
    }
}

#[test]
fn callable_composed_guard_edges_contract_and_call_operands_rejoin_checked_source() {
    for mutation in 0..7 {
        let mut checked = checked(SOURCE);
        let callee = &mut checked.facts.flow.terminal_unit_effects.composed_machines[0];
        match mutation {
            0 => callee.contract_report_fingerprint ^= 1,
            1 => {
                let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard,
                    ..
                } = &mut callee.states[0].terminator
                else {
                    panic!("conditional");
                };
                *guard = checked_trees::CheckedScalarExpression::Boolean(Box::new(
                    checked_trees::CheckedBooleanExpression::Constant(false),
                ));
            }
            2 => {
                let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
                    when_true,
                    when_false,
                    ..
                } = &mut callee.states[0].terminator
                else {
                    panic!("conditional");
                };
                std::mem::swap(&mut when_true.target_state, &mut when_false.target_state);
            }
            3 => {
                let checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_contract_report_fingerprint,
                    ..
                } = &mut callee.states[1].operations[0]
                else {
                    panic!("boundary");
                };
                *target_contract_report_fingerprint ^= 1;
            }
            4 => {
                let checked_trees::CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } =
                    &mut callee.states[2].operations[0]
                else {
                    panic!("call");
                };
                coordinate.statement_index += 1;
            }
            5 => callee.attachment_type_identity = Some("named(name(Root))".to_owned()),
            6 => callee.attachment_type_identity = None,
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_call_to_composed_body_retains_target_state_contract_and_reach() {
    for mutation in 0..3 {
        let mut checked = checked(SOURCE);
        let plans = &mut checked.facts.flow.terminal_unit_effects;
        let leaf_state = plans.composed_machines[0].states[1].state;
        let caller = plans
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        checked_trees::CheckedUnitEffectOperationPlan::CallUnit { .. }
                    )
                })
            })
            .unwrap();
        let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            ..
        } = &mut caller.operations[0]
        else {
            panic!("caller operation");
        };
        match mutation {
            0 => *target_state = leaf_state,
            1 => *target_contract_report_fingerprint ^= 1,
            2 => *service_reach = Default::default(),
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_caller_transfers_linear_claim_into_composed_callee() {
    let source = r#"
        pub data Receipt [linear] { value: u64; }
        boundary machine Receipt::settle(self) ensures true;
        data Helper {}
        machine Helper::choose(enabled: bool, receipt: Receipt) {
            transition enabled { true -> yes(receipt) false -> no(receipt) }
            state yes(receipt: Receipt) { receipt.settle(); }
            state no(receipt: Receipt) { receipt.settle(); }
        }
        data Root {}
        machine Root::enter(enabled: bool, receipt: Receipt) {
            Helper::choose(enabled, receipt);
        }
    "#;
    let baseline = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&baseline, "Root::enter").unwrap();
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.blocks.len() == 3)
        .unwrap();
    assert_eq!(callee.entry_claims.len(), 1);
    assert_eq!(callee.structural_parameters.len(), 1);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.blocks.len() == 1)
        .unwrap();
    let terminal_psi::OperationKind::CallUnit {
        callee: target,
        claim_transfers,
        ..
    } = &caller.blocks[0].operations[0].kind
    else {
        panic!("call");
    };
    assert_eq!(*target, callee.id);
    assert_eq!(claim_transfers.len(), 1);
    for mutation in 0..2 {
        let mut checked = baseline.clone();
        let callee = &mut checked.facts.flow.terminal_unit_effects.composed_machines[0];
        if mutation == 0 {
            let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                ..
            } = &mut callee.states[0].terminator
            else {
                panic!("conditional");
            };
            when_true.transfers[0].source =
                checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
        } else {
            callee.states[1].entry_claims[0].claim_identity =
                language_semantics::PermissionClaimIdentity::Unknown;
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err(),
            "mutation {mutation}"
        );
    }
}
