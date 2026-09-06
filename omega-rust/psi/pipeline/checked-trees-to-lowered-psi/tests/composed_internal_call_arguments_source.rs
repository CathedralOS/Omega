//! Composed Unit calls execute their scalar operands and their observable bodies.

use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

const HELPERS: &str = r#"
    machine identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
    data Scalar {}
    machine Scalar::identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
    machine identity16(input: u16) -> u16
    requires 0u16 == 0u16
    ensures 0u16 == 0u16
    { input }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn artifact(checked: &checked_trees::CheckedTrees, state_count: usize) -> (Vec<u8>, Vec<u8>) {
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let states = checked.typed.machine_states(root);
    assert_eq!(
        states.len(),
        state_count,
        "no synthetic authored control states"
    );
    for state in states {
        assert!(
            checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .all(|statement| !matches!(statement, StatementNode::LocalData(_))),
            "computed operands do not manufacture authored temporaries"
        );
    }
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, plan)| {
                plan.machine == root.symbol
                    && matches!(
                        plan.role,
                        CheckedScalarExpressionRole::UnitCallArgument { .. }
                    )
            }),
        "root retains ordinary Unit argument computations"
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("composed internal Unit operand and body lowering");
    if state_count == 4 {
        let helper = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "identity16")
            .unwrap();
        let mut callers = Vec::new();
        let mut targets = std::collections::BTreeSet::new();
        for occurrence in &lowered.source_call_occurrences {
            // Scalar emission records CheckedScalarCallPlan::target_machine,
            // not the authored entry-state target used by ordinary Unit calls.
            if occurrence.source_target != helper.symbol {
                continue;
            }
            if !callers.contains(&occurrence.source_state) {
                callers.push(occurrence.source_state);
            }
            let operation = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == occurrence.terminal_operation)
                .unwrap();
            let terminal_psi::OperationKind::Call { callee, .. } = operation.kind else {
                panic!("identity16 source joins identify scalar calls");
            };
            targets.insert(callee);
        }
        assert!(
            callers.len() >= 2,
            "shared helper is called from root prefix and Unit bodies"
        );
        assert_eq!(
            targets.len(),
            1,
            "one source helper has one Terminal machine identity across the closure"
        );
        assert_eq!(
            lowered
                .semantic_module
                .machines
                .iter()
                .filter(|machine| targets.contains(&machine.id))
                .count(),
            1
        );
    }
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::CallUnit { .. }
            )),
        "ordinary Unit calls remain actual terminal calls"
    );
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    (semantic, evidence)
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[derive(Default)]
struct ObserveCalls(Vec<Vec<TerminalScalarValue>>);

impl TerminalEffectHandler for ObserveCalls {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("observable sink effect");
        };
        assert!(structural_arguments.is_empty());
        self.0.push(arguments.clone());
        Ok(())
    }
}

fn execute(
    artifact: &(Vec<u8>, Vec<u8>),
    arguments: &[bool],
) -> (TerminalExecutionStatus, Vec<Vec<TerminalScalarValue>>) {
    let arguments = arguments
        .iter()
        .copied()
        .map(TerminalScalarValue::Boolean)
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &arguments,
        &[],
    )
    .unwrap();
    let mut observer = ObserveCalls::default();
    let status = execution
        .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
        .unwrap();
    (status, observer.0)
}

fn ordinary_source(qualified: bool, prefix: bool) -> (String, usize) {
    let owner = if qualified { "Relay::" } else { "" };
    let control = if prefix {
        format!(
            r#"
            {owner}relay(identity16(41u16), identity16(43u16));
            transition first {{ true -> dispatch(second) _ -> no() }}
            state dispatch(second: bool) {{ transition second {{ true -> yes() _ -> no() }} }}
        "#
        )
    } else {
        "transition first { true -> yes() _ -> no() }".into()
    };
    let parameters = if prefix {
        "first: bool, second: bool"
    } else {
        "first: bool"
    };
    (
        format!(
            r#"
        {HELPERS}
        boundary trait Sink {{ machine finish(first: u16, second: u16); }}
        data Relay {{}}
        machine {owner}forward(first: u16, second: u16) {{
            Sink::finish(identity16(first), identity16(second));
        }}
        machine {owner}relay(first: u16, second: u16) {{
            Sink::finish(first, second);
            {owner}forward(identity16(second), identity16(first));
        }}
        machine {owner}other(first: u16, second: u16) {{ Sink::finish(second, first); }}
        data Main {{}}
        machine Main::main({parameters}) {{
            {control}
            state yes() {{
                {owner}relay((Scalar::identity(identity(255u8)) as u16) + 1u16, identity(7u8) as u16);
            }}
            state no() {{
                {owner}relay(Scalar::identity(identity(3u8)) as u16, identity(19u8) as u16);
            }}
        }}
    "#
        ),
        if prefix { 4 } else { 3 },
    )
}

#[test]
fn ordinary_unit_leaf_bodies_preserve_parameters_statement_order_and_transitive_effects() {
    for qualified in [false, true] {
        let (source, states) = ordinary_source(qualified, false);
        let artifact = artifact(&checked(&source), states);
        for selected in [false, true] {
            let (status, effects) = execute(&artifact, &[selected]);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let [first, second] = if selected { [256, 7] } else { [3, 19] };
            assert_eq!(
                effects,
                vec![
                    vec![unsigned(first), unsigned(second)],
                    vec![unsigned(second), unsigned(first)]
                ],
                "both observable statements execute once inside the selected ordinary callee"
            );
        }
    }
}

#[test]
fn computed_unit_prefix_keeps_original_boolean_namespace_for_nested_control() {
    for qualified in [false, true] {
        let (source, states) = ordinary_source(qualified, true);
        let artifact = artifact(&checked(&source), states);
        for (first, second) in [(false, false), (false, true), (true, false), (true, true)] {
            let (status, effects) = execute(&artifact, &[first, second]);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let [left, right] = if first && second { [256, 7] } else { [3, 19] };
            assert_eq!(
                effects,
                vec![
                    vec![unsigned(41), unsigned(43)],
                    vec![unsigned(43), unsigned(41)],
                    vec![unsigned(left), unsigned(right)],
                    vec![unsigned(right), unsigned(left)],
                ],
                "prefix completes before the guard and does not replace its retained input values"
            );
        }
    }
}

#[test]
fn internal_unit_body_establishes_affine_locals_and_discards_them_in_reverse_order() {
    let source = format!(
        r#"
        {HELPERS}
        data Empty {{}}
        boundary trait Sink {{ machine finish(first: u16, second: u16); }}
        data Relay {{}}
        machine Relay::cleanup(first: u16, second: u16) {{
            let one: Empty = Empty {{}};
            let two: Empty = Empty {{}};
            Sink::finish(first, second);
        }}
        data Main {{}}
        machine Main::main(selected: bool) {{
            transition selected {{ true -> yes() _ -> no() }}
            state yes() {{ Relay::cleanup(identity16(17u16), identity16(23u16)); }}
            state no() {{ Relay::cleanup(identity16(23u16), identity16(17u16)); }}
        }}
    "#
    );
    let artifact = artifact(&checked(&source), 3);
    let module = decode_module(&artifact.0).unwrap();
    let bodies = module
        .machines
        .iter()
        .filter(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::EstablishTrivialAffineLocal { .. }
                    )
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bodies.len(),
        1,
        "both leaves share the complete cleanup-bearing Unit body"
    );
    let established = bodies[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } => {
                Some(destination)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(established.len(), 2);
    let discards = bodies[0]
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some(trivial_affine_discards),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(discards.len(), 1);
    assert_eq!(discards[0], &[established[1], established[0]]);
    for selected in [false, true] {
        let (status, effects) = execute(&artifact, &[selected]);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        let values = if selected { [17, 23] } else { [23, 17] };
        assert_eq!(effects, vec![values.map(unsigned).to_vec()]);
    }
}

#[test]
fn selected_unit_call_arguments_short_circuit_before_entering_the_observable_callee() {
    for (first, second) in [(false, true), (true, false), (false, false)] {
        let source = format!(
            r#"
            machine abort() -> bool crashes Abort {{ crash Abort; }}
            machine trap() -> bool crashes Trap {{ crash Trap; }}
            boundary trait Sink {{ machine finish(first: bool, second: bool); }}
            data Relay {{}}
            machine Relay::consume(first: bool, second: bool) {{ Sink::finish(first, second); }}
            data Main {{}}
            machine Main::main(selected: bool) crashes Abort crashes Trap {{
                transition selected {{ true -> yes() _ -> no() }}
                state yes() {{ Relay::consume({first} && abort(), {second} || trap()); }}
                state no() {{ Relay::consume(false, true); }}
            }}
        "#
        );
        let artifact = artifact(&checked(&source), 3);
        for selected in [false, true] {
            let (status, effects) = execute(&artifact, &[selected]);
            let cause = if !selected {
                None
            } else if first {
                Some(terminal_psi::CrashCause::Abort)
            } else if !second {
                Some(terminal_psi::CrashCause::Trap)
            } else {
                None
            };
            if let Some(cause) = cause {
                assert!(
                    matches!(status, TerminalExecutionStatus::Crashed(crash) if crash.cause == cause)
                );
                assert!(
                    effects.is_empty(),
                    "argument crash never enters the Unit callee"
                );
            } else {
                assert_eq!(
                    status,
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                );
                assert_eq!(
                    effects,
                    vec![vec![
                        TerminalScalarValue::Boolean(false),
                        TerminalScalarValue::Boolean(true)
                    ]]
                );
            }
        }
    }
}

#[test]
fn selected_unit_first_argument_crash_wins_over_later_call_and_callee_effects() {
    for (first, second, cause) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        let source = format!(
            r#"
            machine first() -> u8 crashes {first} {{ crash {first}; }}
            machine second() -> u8 crashes {second} {{ crash {second}; }}
            boundary trait Sink {{ machine finish(first: u16, second: u16); }}
            machine consume(first: u16, second: u16) {{ Sink::finish(first, second); }}
            data Main {{}}
            machine Main::main(selected: bool) crashes Abort crashes Trap {{
                transition selected {{ true -> yes() _ -> no() }}
                state yes() {{ consume(first() as u16, second() as u16); }}
                state no() {{ consume(second() as u16, first() as u16); }}
            }}
        "#
        );
        let artifact = artifact(&checked(&source), 3);
        for selected in [false, true] {
            let expected = if selected {
                cause
            } else if cause == terminal_psi::CrashCause::Abort {
                terminal_psi::CrashCause::Trap
            } else {
                terminal_psi::CrashCause::Abort
            };
            let (status, effects) = execute(&artifact, &[selected]);
            assert!(
                matches!(status, TerminalExecutionStatus::Crashed(crash) if crash.cause == expected)
            );
            assert!(effects.is_empty());
        }
    }
}

#[test]
fn transitive_unit_callee_suspension_metadata_is_retained_and_validated() {
    let (source, states) = ordinary_source(true, false);
    let mut checked = checked(&source);
    artifact(&checked, states);
    let owner = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Relay::forward")
        .unwrap();
    let owner_symbol = owner.symbol;
    let owner_state = checked.typed.machine_states(owner)[0].symbol;
    let helper = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity16")
        .unwrap();
    let helper_symbol = helper.symbol;
    let helper_type = checked
        .typed
        .state_parameters(&checked.typed.machine_states(helper)[0])[0]
        .type_reference;
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").unwrap();
    assert!(lowered.semantic_module.suspension_call_plans.is_empty());
    let occurrence = lowered
        .source_call_occurrences
        .iter()
        .find(|occurrence| {
            occurrence.source_state == owner_state && occurrence.source_target == helper_symbol
        })
        .unwrap();
    // As in the scalar suspension-frontier tests, inject a checked crossing
    // at a real call to isolate finalization's source-owner selection.
    checked
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: owner_symbol,
            state: owner_state,
            statement_index: occurrence.statement_index,
            call_ordinal: occurrence.call_ordinal,
            target: helper_symbol,
            receiver: None,
            effective: language_semantics::CarryPolicy::PERMISSIVE,
            live_values: vec![checked_trees::SuspensionCrossingLiveValueFact {
                type_reference: helper_type,
                storage: checked_trees::SuspensionCrossingStorage::CallArgument,
                origin: checked_trees::SuspensionCrossingValueOrigin::CallArgument { position: 0 },
                claims: Vec::new(),
                effective: language_semantics::CarryPolicy::PERMISSIVE,
            }],
        });
    let artifact = artifact(&checked, states);
    let module = decode_module(&artifact.0).unwrap();
    let [plan] = module.suspension_call_plans.as_slice() else {
        panic!("callee-only crossing must not disappear outside the root owner");
    };
    let [site] = module.suspension_call_sites.as_slice() else {
        panic!("one exact callee suspension call site");
    };
    assert_eq!(plan.operation, occurrence.terminal_operation);
    assert_eq!(site.operation, plan.operation);
    assert_eq!(
        site.frontier_commitment,
        terminal_psi::suspension_frontier_commitment(plan)
    );
    assert_eq!(plan.live_values.len(), 1);
    assert_eq!(
        plan.live_values[0].storage,
        terminal_psi::TerminalSuspensionStorage::CallArgument
    );
    checked
        .facts
        .carry
        .suspension_crossings
        .last_mut()
        .unwrap()
        .receiver = Some(owner_symbol);
    assert!(
        matches!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main"),
            Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "receiver-bearing suspension frontier lacks an exact Terminal receiver place join"
            ))
        ),
        "callee-only unsupported metadata must reject rather than be omitted"
    );
}

#[test]
fn outer_and_transitive_unit_calls_reject_target_arity_and_operand_source_drift() {
    let (source, states) = ordinary_source(true, true);
    let checked = checked(&source);
    artifact(&checked, states);
    let replacement = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Relay::other")
        .unwrap();
    let replacement_state = checked.typed.machine_states(replacement)[0].symbol;
    let mut covered = Vec::new();
    for machine in checked.typed.machines() {
        if !matches!(
            machine.name.as_str(),
            "Main::main" | "Relay::relay" | "Relay::forward"
        ) {
            continue;
        }
        for state in checked.typed.machine_states(machine) {
            for (index, statement) in checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                covered.push(machine.name.as_str());
                for mutation in 0..3 {
                    let mut changed = checked.clone();
                    let StatementNode::Call(changed_call) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(state.statement_nodes)[index]
                    else {
                        unreachable!();
                    };
                    match mutation {
                        0 => changed_call.target_symbol = replacement_state,
                        1 => changed_call.arguments = arena::HandleSpan::empty(),
                        2 => {
                            let arguments = checked
                                .typed
                                .statement_table
                                .expression_handles(call.arguments);
                            changed
                                .typed
                                .statement_table
                                .set_expression_handle_at_offset(call.arguments, 0, arguments[1]);
                        }
                        _ => unreachable!(),
                    }
                    assert!(
                        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main")
                            .is_err(),
                        "{}: outer/target call mutation={mutation}",
                        machine.name.as_str()
                    );
                }
            }
        }
    }
    for owner in ["Main::main", "Relay::relay", "Relay::forward"] {
        assert!(covered.contains(&owner));
    }
    let plans = &checked.facts.values.scalar_computations;
    for (_, node) in plans.nodes.iter() {
        let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
            continue;
        };
        let authored = checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .authored_expression;
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .control
            .calls
            .get_mut(source_call)
            .authored_expression = arena::Handle::invalid();
        assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err());
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored) else {
            unreachable!();
        };
        if call.receiver.is_valid() {
            let mut changed = checked.clone();
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(call.receiver)
            else {
                unreachable!();
            };
            path.symbol = symbols::SymbolHandle::invalid();
            assert!(checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err());
        }
    }
}
