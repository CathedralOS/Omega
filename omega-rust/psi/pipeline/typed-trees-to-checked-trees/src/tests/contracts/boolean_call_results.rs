use super::*;

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
            "let mut current: bool = value; let saved: bool = identity(!current); saved",
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
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedScalarExpressionRole as Role,
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
    let (binding_handle, binding) = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find(|(_, binding)| {
            binding.state == state
                && binding.role
                    == (Role::CallArgument {
                        binding_ordinal: 0,
                        argument_ordinal: 0,
                    })
        })
        .unwrap();
    let binding = binding.clone();
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
                let plans = &mut facts.values.scalar_expressions;
                plans.source_bindings.append(binding.clone());
            }
            6 => {
                let plans = &mut facts.values.scalar_expressions;
                let mut symbols = plans
                    .binding_symbols
                    .span_or_empty(binding.symbols)
                    .to_vec();
                symbols.swap(0, 1);
                plans.source_bindings.get_mut(binding_handle).symbols =
                    plans.binding_symbols.insert_many(symbols);
            }
            7 => {
                facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(binding_handle)
                    .destination = machine.symbol
            }
            8 => {
                let plan = facts
                    .values
                    .scalar_expressions
                    .expressions
                    .iter_mut()
                    .find(|plan| plan.state == state && plan.role == binding.role)
                    .unwrap();
                plan.expression =
                    Scalar::Boolean(Box::new(Boolean::Not(Box::new(Boolean::Parameter {
                        position: 1,
                    }))));
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
