use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

const HELPERS: &str = "
boundary trait Host { machine finish(value: bool) reaches Host; }
machine identity(value: bool) -> bool ensures result == value { value }
machine flip(value: bool) -> bool ensures result == !value { !value }
machine equal(marker: u16, left: bool, right: bool) -> bool
ensures result == (left == right) { left == right }
machine constant() -> bool ensures result == true { true }
";

fn source(body: &str, guarantee: &str) -> String {
    format!(
        "{HELPERS}
        machine compute(value: bool, other: bool) -> bool
        ensures {guarantee}
        reaches Host {{ {body} }}"
    )
}

#[test]
fn call_produced_boolean_guarantees_compose_with_captured_locals() {
    for (body, guarantee) in [
        (
            "let saved: bool = identity(!value); saved",
            "result == !value",
        ),
        (
            "let saved: bool = flip(value); Host::finish(false); saved",
            "result == !value",
        ),
        (
            "let first: bool = identity(value); Host::finish(false); let saved: bool = flip(first); saved",
            "result == !value",
        ),
        (
            "let first: bool = !value; let saved: bool = identity(first); saved",
            "result == !value",
        ),
        (
            "let saved: bool = equal(7u16, other, value); saved",
            "result == (other == value)",
        ),
        ("let saved: bool = constant(); saved", "result == true"),
        (
            "let mut current: bool = value; let saved: bool = identity(!current); saved",
            "result == !value",
        ),
    ] {
        lower_typed_trees(parse_typed_trees(&source(body, guarantee)))
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn call_produced_boolean_results_need_real_guarantees_and_independent_requires() {
    for (helper, body, failure) in [
        (
            "machine bare(value: bool) -> bool { value }",
            "let saved: bool = bare(!value); saved",
            "ensures contract for exit from compute",
        ),
        (
            "machine wrong(value: bool) -> bool ensures result == value { !value }",
            "let saved: bool = wrong(!value); saved",
            "ensures contract for exit from wrong",
        ),
        (
            "machine guarded(value: bool) -> bool requires value\nensures result == value { value }",
            "let saved: bool = guarded(!value); saved",
            "requires contract for call guarded",
        ),
        (
            "",
            "let saved: bool = identity(!other); saved",
            "ensures contract for exit from compute",
        ),
        (
            "",
            "let mut current: bool = value; current = other; let saved: bool = identity(!current); saved",
            "ensures contract for exit from compute",
        ),
        (
            "machine named(result: bool, value: bool) -> bool requires result == value\nensures result == value { false }",
            "let saved: bool = named(!value, !value); saved",
            "ensures contract for exit from compute",
        ),
    ] {
        let program = parse_typed_trees(&format!("{helper}\n{}", source(body, "result == !value")));
        let diagnostics = lower_typed_trees(program)
            .expect_err("call contracts must not be invented or circular");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(failure)),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn call_produced_boolean_guarantees_reject_altered_call_and_argument_custody() {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarComputationKind as Computation,
        CheckedScalarExpression as Scalar, CheckedScalarExpressionRole as Role,
    };
    let checked = lower_typed_trees(parse_typed_trees(&source(
        "let saved: bool = identity(!value); Host::finish(false); saved",
        "result == !value",
    )))
    .expect("saved call result");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compute")
        .unwrap();
    let state = checked.machine_states(machine)[0].symbol;
    let (call_handle, call) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, call)| {
            call.statement_index == 0
                && checked.facts.flow.control.states.iter().any(|(_, flow)| {
                    flow.state_symbol == state
                        && checked
                            .facts
                            .flow
                            .control
                            .calls
                            .span_or_empty(flow.calls)
                            .contains(call)
                })
        })
        .unwrap();
    let call = call.clone();
    let guarantee = checked
        .facts
        .proof
        .contract_fact_refs
        .span_or_empty(call.ensures)[0]
        .fact;
    // Since 8efcd822fb a computed Boolean initializer proves its call
    // guarantee through the scalar computation graph: the call's argument
    // custody is the graph `Call` node's operand and the root's coordinates,
    // so the custody controls below alter that graph. A pure `CallArgument`
    // source binding is not consulted for this proof and mutating it alone
    // does not change the proven equation.
    let computations = &checked.facts.values.scalar_computations;
    let root_handle = computations
        .root_at(state, 0, Role::LocalInitializer { binding_ordinal: 0 })
        .expect("computed call initializer graph")
        .root;
    let Computation::Call { arguments, .. } = &computations.nodes.get(root_handle).kind else {
        panic!("call root");
    };
    let [operand] = computations.operands.span_or_empty(*arguments) else {
        panic!("one scalar operand");
    };
    let operand = *operand;
    let other_call = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(handle, _)| *handle != call_handle)
        .map(|(handle, _)| handle)
        .expect("the Host::finish call");
    for mutation in 0..10 {
        let mut facts = checked.facts.clone();
        match mutation {
            0 => facts.flow.control.calls.get_mut(call_handle).target_symbol = state,
            1 => facts.flow.control.calls.get_mut(call_handle).call_ordinal = 999,
            2 => {
                facts
                    .flow
                    .control
                    .calls
                    .get_mut(call_handle)
                    .statement_index = 1
            }
            3 => facts.flow.control.calls.get_mut(call_handle).ensures = arena::HandleSpan::empty(),
            4 => {
                facts.proof.contract_facts.get_mut(guarantee).owner =
                    checked_trees::ContractProofFactOwner::Machine {
                        machine_symbol: machine.symbol,
                    }
            }
            5 => {
                facts.values.scalar_computations.nodes.get_mut(operand).kind =
                    Computation::Value(Scalar::Boolean(Box::new(Boolean::Not(Box::new(
                        Boolean::Parameter { position: 1 },
                    )))));
            }
            6 => {
                facts.values.scalar_computations.nodes.get_mut(operand).kind =
                    Computation::Value(Scalar::Boolean(Box::new(Boolean::Parameter {
                        position: 0,
                    })));
            }
            7 => {
                let Computation::Call { source_call, .. } = &mut facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(root_handle)
                    .kind
                else {
                    unreachable!("call root");
                };
                *source_call = other_call;
            }
            8 => {
                let root = facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .find(|(_, root)| root.root == root_handle)
                    .map(|(_, root)| root.clone())
                    .unwrap();
                facts.values.scalar_computations.roots.append(root);
            }
            9 => {
                let (flow_handle, flow) = facts
                    .flow
                    .control
                    .states
                    .iter()
                    .find(|(_, flow)| flow.state_symbol == state)
                    .unwrap();
                let mut calls = facts.flow.control.calls.span_or_empty(flow.calls).to_vec();
                calls.push(call.clone());
                facts.flow.control.states.get_mut(flow_handle).calls =
                    facts.flow.control.calls.insert_many(calls);
            }
            _ => unreachable!(),
        }
        assert!(
            crate::checks::check_checked_facts(&checked.typed, &facts).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn boolean_computation_graphs_prove_source_normal_guarantees() {
    // These assertions stop at source checking. Short-circuit graph publication
    // additionally needs independently reconstructed control-merge evidence.
    for (body, guarantee) in [
        (
            "Host::finish(false); identity(!identity(value))",
            "result == !value",
        ),
        (
            "let saved: bool = identity(!identity(value)); Host::finish(false); saved",
            "result == !value",
        ),
        ("Host::finish(false); !identity(value)", "result == !value"),
        (
            "let saved: bool = identity(value) == identity(other); Host::finish(false); saved",
            "result == (value == other)",
        ),
        (
            "let saved: bool = identity(value) || identity(other); Host::finish(false); saved",
            "result == (value || other)",
        ),
        (
            "let saved: bool = identity(value) && identity(other); Host::finish(false); saved",
            "result == (value && other)",
        ),
    ] {
        lower_typed_trees(parse_typed_trees(&source(body, guarantee)))
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:#?}"));
    }
}

#[test]
fn boolean_computation_graphs_reject_altered_application_and_call_custody() {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarComputationKind as Computation,
        CheckedScalarExpression as Scalar, CheckedScalarExpressionRole as Role,
    };
    let checked = lower_typed_trees(parse_typed_trees(&source(
        "let saved: bool = identity(value) == identity(other); Host::finish(false); saved",
        "result == (value == other)",
    )))
    .expect("source graph guarantee");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compute")
        .unwrap();
    let state = checked.machine_states(machine)[0].symbol;
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state, 0, Role::LocalInitializer { binding_ordinal: 0 })
        .expect("whole initializer graph")
        .clone();
    let Computation::Apply { operands, .. } = &plans.nodes.get(root.root).kind else {
        panic!("equality application root");
    };
    let operands = plans.operands.span(*operands).unwrap().to_vec();
    assert_eq!(operands.len(), 2);
    let Computation::Call {
        source_call: other_call,
        ..
    } = plans.nodes.get(operands[1]).kind
    else {
        panic!("second call operand");
    };
    for mutation in 0..6 {
        let mut facts = checked.facts.clone();
        let plans = &mut facts.values.scalar_computations;
        match mutation {
            0 | 5 => {
                let mut altered = operands.clone();
                if mutation == 0 {
                    altered.swap(0, 1);
                } else {
                    altered[0] = root.root;
                }
                let altered = plans.operands.insert_many(altered);
                let Computation::Apply { operands, .. } = &mut plans.nodes.get_mut(root.root).kind
                else {
                    unreachable!();
                };
                *operands = altered;
            }
            1 => {
                let Computation::Call { source_call, .. } =
                    &mut plans.nodes.get_mut(operands[0]).kind
                else {
                    unreachable!();
                };
                *source_call = other_call;
            }
            2 => {
                let Computation::Apply { expression, .. } =
                    &mut plans.nodes.get_mut(root.root).kind
                else {
                    unreachable!();
                };
                *expression = Scalar::Boolean(Box::new(Boolean::Equal {
                    left: Box::new(Boolean::Parameter { position: 1 }),
                    right: Box::new(Boolean::Parameter { position: 0 }),
                }));
            }
            3 => {
                let other_source = plans.nodes.get(operands[0]).authored_root;
                let Computation::Apply {
                    source_expression, ..
                } = &mut plans.nodes.get_mut(root.root).kind
                else {
                    unreachable!();
                };
                *source_expression = other_source;
            }
            4 => {
                plans.roots.append(root.clone());
            }
            _ => unreachable!(),
        }
        assert!(
            crate::checks::check_checked_facts(&checked.typed, &facts).is_err(),
            "application mutation {mutation}"
        );
    }
}

#[test]
fn boolean_computation_graphs_reject_altered_short_circuit_custody() {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarComputationKind as Computation,
        CheckedScalarExpression as Scalar, CheckedScalarExpressionRole as Role,
    };
    for operator in ["&&", "||"] {
        let checked = lower_typed_trees(parse_typed_trees(&source(
            &format!("let saved: bool = identity(value) {operator} identity(other); Host::finish(false); saved"),
            &format!("result == (value {operator} other)"),
        )))
        .expect("source short-circuit guarantee");
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "compute")
            .unwrap();
        let state = checked.machine_states(machine)[0].symbol;
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .root_at(state, 0, Role::LocalInitializer { binding_ordinal: 0 })
            .expect("whole short-circuit graph")
            .root;
        let Computation::Select {
            when_true,
            when_false,
            ..
        } = plans.nodes.get(root).kind
        else {
            panic!("short-circuit select root");
        };
        let skipped = if operator == "&&" {
            when_false
        } else {
            when_true
        };
        for mutation in 0..2 {
            let mut facts = checked.facts.clone();
            let plans = &mut facts.values.scalar_computations;
            if mutation == 0 {
                plans.nodes.get_mut(skipped).kind = Computation::Value(Scalar::Boolean(Box::new(
                    Boolean::Constant(operator == "&&"),
                )));
            } else {
                let Computation::Select {
                    when_true,
                    when_false,
                    ..
                } = &mut plans.nodes.get_mut(root).kind
                else {
                    unreachable!();
                };
                std::mem::swap(when_true, when_false);
            }
            assert!(
                crate::checks::check_checked_facts(&checked.typed, &facts).is_err(),
                "{operator} select mutation {mutation}"
            );
        }
    }
}

#[test]
fn boolean_computation_graphs_capture_mutable_values_at_the_selected_read() {
    for (before_capture, admitted) in [("", true), ("current = other;", false)] {
        let program = parse_typed_trees(&source(
            &format!(
                "let mut current: bool = value; {before_capture} let saved: bool = identity(!identity(current)); current = other; Host::finish(false); saved"
            ),
            "result == !value",
        ));
        let result = lower_typed_trees(program);
        assert_eq!(result.is_ok(), admitted, "{before_capture}");
        if let Err(diagnostics) = result {
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("ensures contract for exit from compute")),
                "{diagnostics:#?}"
            );
        }
    }
}
