//! Scalar-result roots share primitive referents with their computed callees.

use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarExpression,
    CheckedTrees,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralAccess, TerminalMachineResult};

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

const SOURCE: &str = r#"
machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
machine first(left: u64, right: u64) -> u64 { left }
machine enter(before: u64, slot: &mut u64, after: u64) -> u64 {
    let answer: u64 = first(stamp(&mut slot, before), stamp(&mut slot, after));
    answer
}
"#;

#[test]
fn scalar_root_preserves_ordered_borrowed_computations() {
    execute(
        SOURCE,
        &[unsigned(7), unsigned(11)],
        ExecutionExpectations {
            result: TerminalExecutionResult::Scalar(unsigned(7)),
            observations: &[201, 7, 11],
            stamp_calls: 2,
            stamp_invocations: 2,
            machines: 3,
        },
    );
}

#[test]
fn scalar_root_finalizes_remainder_and_narrowing_after_borrowed_computations() {
    let source = SOURCE
        .replace("after: u64) -> u64", "after: u64) -> u8")
        .replace("\n    answer\n", "\n    (answer % 4u64) as u8\n");
    let module = execute(
        &source,
        &[unsigned(7), unsigned(11)],
        ExecutionExpectations {
            result: TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(3),
            }),
            observations: &[201, 7, 11],
            stamp_calls: 2,
            stamp_invocations: 2,
            machines: 3,
        },
    );
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    for narrowing in [false, true] {
        assert!(
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| if narrowing {
                    matches!(operation.kind, OperationKind::IntegerExactCast { .. })
                } else {
                    matches!(operation.kind, OperationKind::ExactIntegerRemainder { .. })
                }),
            "root retains the arithmetic operation whose proof must finalize, narrowing={narrowing}"
        );
    }
}

#[test]
fn scalar_root_short_circuit_only_invokes_the_selected_borrowed_call() {
    let source = r#"
        machine stamp(value: &mut u64, number: u64) -> bool { value = number; true }
        machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {
            let answer: bool = enabled && stamp(&mut slot, number);
            answer
        }
    "#;
    for enabled in [false, true] {
        execute(
            source,
            &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
            ExecutionExpectations {
                result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(enabled)),
                observations: if enabled { &[201, 7] } else { &[201] },
                stamp_calls: 1,
                stamp_invocations: u64::from(enabled),
                machines: 2,
            },
        );
    }
}

#[test]
fn scalar_root_folded_short_circuit_retains_the_selected_source_scope() {
    let source = r#"
        machine stamp(value: &mut u64, number: u64) -> bool { value = number; true }
        machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {
            let answer: bool = (false && enabled) && stamp(&mut slot, number);
            let written: bool = stamp(&mut slot, number);
            answer
        }
    "#;
    for condition in ["false", "true && false", "false || false", "1u64 > 2u64"] {
        let source = source.replace("false && enabled", &format!("({condition}) && enabled"));
        execute(
            &source,
            &[TerminalScalarValue::Boolean(true), unsigned(7)],
            ExecutionExpectations {
                result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false)),
                observations: &[201, 7],
                stamp_calls: 1,
                stamp_invocations: 1,
                machines: 2,
            },
        );
    }
}

#[test]
fn scalar_root_known_guard_result_preserves_left_effects() {
    let source = r#"
        machine stamp(value: &mut u64, number: u64, answer: bool) -> bool {
            value = number;
            answer
        }
        machine first(left: bool, right: bool) -> bool { left }
        machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {
            let answer: bool = RESULT;
            TAIL
            answer
        }
    "#;
    for (guard, known_result, writes_guard) in [
        ("(stamp(&mut slot, number, enabled) && false)", false, true),
        ("(stamp(&mut slot, number, enabled) || true)", true, true),
        ("!(stamp(&mut slot, number, enabled) && false)", true, true),
        ("!(stamp(&mut slot, number, enabled) || true)", false, true),
        (
            "((stamp(&mut slot, number, enabled) && false) && enabled)",
            false,
            true,
        ),
        (
            "((stamp(&mut slot, number, enabled) || true) || enabled)",
            true,
            true,
        ),
        ("(enabled && false)", false, false),
        ("(enabled || true)", true, false),
        ("!(enabled && false)", true, false),
        ("!(enabled || true)", false, false),
    ] {
        for conjunction in [false, true] {
            let invokes_right = known_result == conjunction;
            let result = if conjunction { known_result } else { true };
            for (result_expression, tail, machines) in [
                (
                    "GUARD OPERATOR stamp(&mut slot, 11u64, true)",
                    "let written: bool = stamp(&mut slot, 13u64, true);",
                    2,
                ),
                (
                    "first(GUARD OPERATOR stamp(&mut slot, 11u64, true), stamp(&mut slot, 13u64, true))",
                    "",
                    3,
                ),
            ] {
                let source = source
                    .replace("RESULT", result_expression)
                    .replace("TAIL", tail)
                    .replace("GUARD", guard)
                    .replace("OPERATOR", if conjunction { "&&" } else { "||" });
                for enabled in [false, true] {
                    execute(
                        &source,
                        &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
                        ExecutionExpectations {
                            result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                                result,
                            )),
                            observations: match (writes_guard, invokes_right) {
                                (true, true) => &[201, 7, 11, 13],
                                (true, false) => &[201, 7, 13],
                                (false, true) => &[201, 11, 13],
                                (false, false) => &[201, 13],
                            },
                            stamp_calls: 1 + usize::from(writes_guard) + usize::from(invokes_right),
                            stamp_invocations: 1
                                + u64::from(writes_guard)
                                + u64::from(invokes_right),
                            machines,
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_root_strict_guard_retains_flow_call_correspondence() {
    let source = r#"
        machine stamp(value: &mut u64, number: u64, answer: bool) -> bool {
            value = number;
            answer
        }
        machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {
            let answer: bool = ((stamp(&mut slot, number, enabled) && false) == false)
                || stamp(&mut slot, 11u64, true);
            let written: bool = stamp(&mut slot, 13u64, true);
            answer
        }
    "#;
    for enabled in [false, true] {
        execute(
            source,
            &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
            ExecutionExpectations {
                result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
                observations: &[201, 7, 13],
                stamp_calls: 3,
                stamp_invocations: 2,
                machines: 2,
            },
        );
    }
}

#[test]
fn scalar_root_comparison_guards_preserve_skipped_and_evaluated_calls() {
    for (condition, constant) in [
        ("1u64 > 2u64", false),
        ("2u64 >= 1u64", true),
        ("2u64 < 1u64", false),
        ("1u64 <= 2u64", true),
        ("1u64 == 2u64", false),
        ("1u64 != 2u64", true),
        ("(1u64 as u8) > 2u8", false),
        ("(1u64 + 1u64) == 2u64", true),
        ("!((1u64 > 2u64) && enabled)", true),
        ("!((1u64 < 2u64) || enabled)", false),
    ] {
        for conjunction in [false, true] {
            let operator = if conjunction { "&&" } else { "||" };
            let source = format!(
                r#"
                machine stamp(value: &mut u64, number: u64) -> bool {{ value = number; true }}
                machine enter(enabled: bool, slot: &mut u64, number: u64) -> bool {{
                    let answer: bool = (({condition}) {operator} enabled)
                        {operator} stamp(&mut slot, number);
                    let written: bool = stamp(&mut slot, number);
                    answer
                }}
                "#,
            );
            for enabled in [false, true] {
                let invokes_first = if conjunction {
                    constant && enabled
                } else {
                    !constant && !enabled
                };
                execute(
                    &source,
                    &[TerminalScalarValue::Boolean(enabled), unsigned(7)],
                    ExecutionExpectations {
                        result: TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                            !conjunction || invokes_first,
                        )),
                        observations: &[201, 7],
                        stamp_calls: 1 + usize::from(constant == conjunction),
                        stamp_invocations: 1 + u64::from(invokes_first),
                        machines: 2,
                    },
                );
            }
        }
    }
}

#[test]
fn ordinary_unit_consumer_uses_the_same_scalar_graph_call_closure() {
    let source = format!(
        r#"{}
        machine enter(before: u64, slot: &mut u64, after: u64) {{
            let answer: u64 = calculate(before, &mut slot, after);
            slot = answer;
        }}
    "#,
        SOURCE.replace("machine enter(", "machine calculate(")
    );
    execute(
        &source,
        &[unsigned(7), unsigned(11)],
        ExecutionExpectations {
            result: TerminalExecutionResult::Unit,
            observations: &[201, 7, 11, 7],
            stamp_calls: 2,
            stamp_invocations: 2,
            machines: 4,
        },
    );
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

struct ExecutionExpectations<'values> {
    result: TerminalExecutionResult,
    observations: &'values [u128],
    stamp_calls: usize,
    stamp_invocations: u64,
    machines: usize,
}

fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
    expected: ExecutionExpectations<'_>,
) -> terminal_psi::TerminalModule {
    let checked = checked(source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "enter")
        .expect("mixed root publishes its complete borrowed scalar call closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("reloaded scalar root artifact verifies independently");
    assert_eq!(module.machines.len(), expected.machines);
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(caller.parameters.len(), arguments.len());
    assert_eq!(caller.structural_parameters.len(), 1);
    let parameter = &caller.structural_parameters[0];
    assert_eq!(parameter.access, StructuralAccess::MutableBorrow);
    assert_eq!(
        parameter.position, 0,
        "structural inputs have dense positions"
    );
    match &expected.result {
        TerminalExecutionResult::Scalar(_) => {
            assert!(matches!(caller.result, TerminalMachineResult::Scalar(_)));
        }
        TerminalExecutionResult::Unit => assert_eq!(caller.result, TerminalMachineResult::Unit),
        other => panic!("unexpected fixture result: {other:?}"),
    }
    let mut stamp_bodies = module.machines.iter().filter(|machine| {
        matches!(machine.result, TerminalMachineResult::Scalar(_))
            && machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::WriteOnlyPrimitiveStore { .. }
                    )
                })
    });
    let stamp = stamp_bodies.next().expect("scalar mutator body");
    assert!(
        stamp_bodies.next().is_none(),
        "stamp has one shared callee identity"
    );
    let stamp_calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(operation.kind,
            OperationKind::CallStructuralScalar { callee, .. } if callee == stamp.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(stamp_calls.len(), expected.stamp_calls);
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(201),
            }],
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut completed = false;
    for _ in 0..128 {
        let status = execution.resume(&mut meter).unwrap();
        let referents = execution.structural_primitive_values();
        assert_eq!(referents.len(), 1);
        assert_eq!(referents[0].argument_index, 0);
        observations.push(referents[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, expected.result);
                completed = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected scalar root execution: {other:?}"),
        }
    }
    assert!(
        completed,
        "scalar root must finish with one-unit replenishments"
    );
    observations.dedup();
    assert_eq!(
        observations,
        expected
            .observations
            .iter()
            .copied()
            .map(unsigned)
            .collect::<Vec<_>>()
    );
    let mut invocations = std::collections::BTreeMap::from([(module.entry, 1)]);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let callee = match operation.kind {
            OperationKind::Call { callee, .. }
            | OperationKind::CallUnit { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. } => callee,
            _ => continue,
        };
        if let Some(usage) = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
        {
            *invocations.entry(callee).or_default() += usage.executions();
        }
    }
    assert_eq!(
        invocations.get(&stamp.id).copied().unwrap_or(0),
        expected.stamp_invocations
    );
    for machine in &module.machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let usage = meter
                .usage()
                .at(terminal_fuel::FuelChargeSite::Operation(operation.id));
            if machine.id == stamp.id {
                assert_eq!(
                    usage.map_or(0, |usage| usage.executions()),
                    expected.stamp_invocations,
                    "each stamp body operation executes once per selected invocation"
                );
            }
            if let Some(usage) = usage {
                assert_eq!(
                    usage.executions(),
                    invocations[&machine.id],
                    "completed operation must not replay across fuel pauses"
                );
                if matches!(
                    operation.kind,
                    OperationKind::CallStructuralScalar { .. }
                        | OperationKind::WriteOnlyPrimitiveStore { .. }
                ) {
                    assert_eq!(
                        usage.units(),
                        usage.executions(),
                        "one unit per call or store"
                    );
                }
            }
        }
    }
    module
}

fn publish_original(source: &str) -> CheckedTrees {
    let checked = checked(source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "enter")
        .expect("unmodified scalar root must publish before testing custody mutations");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("unmodified artifact verifies independently");
    checked
}

fn calls_to(checked: &CheckedTrees, name: &str) -> Vec<CheckedScalarComputationHandle> {
    let target = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("authored callee")
        .symbol;
    let calls = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            matches!(node.kind, CheckedScalarComputationKind::Call { target_machine, .. }
            if target_machine == target)
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(
        !calls.is_empty(),
        "fixture retains computed calls to {name}"
    );
    calls
}

fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        terminal_production::produce_terminal_artifact(checked, "enter").is_err(),
        "publication accepted scalar root custody mutation: {mutation}"
    );
}

#[test]
fn scalar_root_rejects_swapped_dense_scalar_parameter_positions() {
    let original = publish_original(SOURCE);
    let caller = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    let mut changed = original.clone();
    let graph = changed
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter_mut()
        .find(|graph| graph.machine == caller)
        .expect("scalar root graph");
    let state = &mut graph.states[0];
    assert_eq!(
        state
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    state.scalar_parameters.swap(0, 1);
    reject(
        &changed,
        "swapped before/after formal positions across the structural slot",
    );

    let mut changed = original.clone();
    let mut substitutions = 0;
    for call in calls_to(&original, "stamp") {
        let CheckedScalarComputationKind::Call { arguments, .. } = original
            .facts
            .values
            .scalar_computations
            .nodes
            .get(call)
            .kind
        else {
            panic!("computed stamp");
        };
        assert_eq!(arguments.count(), 1);
        let operand = *original
            .facts
            .values
            .scalar_computations
            .operands
            .get(arguments.start());
        let CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
            position,
            ..
        }) = &mut changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(operand)
            .kind
        else {
            panic!("stamp operand retains its scalar parameter");
        };
        assert!(*position < 2);
        *position = 1 - *position;
        substitutions += 1;
    }
    assert_eq!(substitutions, 2);
    reject(&changed, "swapped same-typed scalar actuals");
}

#[test]
fn scalar_root_rejects_missing_and_substituted_value_occurrences() {
    let original = publish_original(SOURCE);
    let stamp = calls_to(&original, "stamp");
    let operands = stamp
        .iter()
        .map(|call| {
            let CheckedScalarComputationKind::Call { arguments, .. } = original
                .facts
                .values
                .scalar_computations
                .nodes
                .get(*call)
                .kind
            else {
                panic!("stamp call");
            };
            *original
                .facts
                .values
                .scalar_computations
                .operands
                .get(arguments.start())
        })
        .collect::<Vec<_>>();
    assert_eq!(operands.len(), 2);
    for substitution in [None, Some(operands[1])] {
        let mut changed = original.clone();
        let replacement = substitution
            .map(|operand| {
                original
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(operand)
                    .value_source
            })
            .unwrap_or_default();
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(operands[0])
            .value_source = replacement;
        reject(
            &changed,
            "missing or another same-typed argument occurrence",
        );
    }
}

#[test]
fn scalar_root_rejects_swapped_same_typed_borrowed_actuals() {
    let source = SOURCE
        .replace(
            "slot: &mut u64, after:",
            "slot: &mut u64, other: &mut u64, after:",
        )
        .replace("stamp(&mut slot, after)", "stamp(&mut other, after)");
    let original = publish_original(&source);
    let mut changed = original.clone();
    let calls = calls_to(&original, "stamp");
    assert_eq!(calls.len(), 2);
    let mut positions = Vec::new();
    for call in calls {
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = original
            .facts
            .values
            .scalar_computations
            .nodes
            .get(call)
            .kind
        else {
            panic!("computed stamp");
        };
        assert_eq!(structural_arguments.count(), 1);
        let argument = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(structural_arguments.start())
            .as_place_mut()
            .expect("retained primitive parameter place");
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            &mut argument.source
        else {
            panic!("root primitive parameter borrow");
        };
        positions.push(*parameter_index);
        assert!(*parameter_index < 2);
        *parameter_index = 1 - *parameter_index;
    }
    positions.sort_unstable();
    assert_eq!(positions, [0, 1]);
    reject(&changed, "swapped same-typed borrowed root actuals");
}

#[test]
fn scalar_root_rejects_missing_callee_type_custody() {
    let mut changed = publish_original(SOURCE);
    let types = &mut changed
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types;
    assert!(
        !types.is_empty(),
        "stamp retains its primitive referent type"
    );
    types.clear();
    reject(&changed, "missing selected callee structural types");
}

#[test]
fn scalar_root_rejects_erased_borrowed_computation() {
    let original = publish_original(SOURCE);
    let mut changed = original.clone();
    let calls = calls_to(&original, "first");
    assert_eq!(calls.len(), 1);
    let node = changed
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(calls[0]);
    node.kind = CheckedScalarComputationKind::Value(CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: node.primitive_type,
    });
    reject(
        &changed,
        "same-typed before value cannot erase the two stamp invocations",
    );
}
