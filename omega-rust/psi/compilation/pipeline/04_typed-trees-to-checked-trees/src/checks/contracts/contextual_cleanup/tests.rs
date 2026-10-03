use super::*;
use crate::tests::front_end::checked_program;

fn check_actuals(source: &str, expected: bool) {
    let mut checked = checked_program(source);
    let target = checked
        .machines()
        .iter()
        .find(|machine| checked.symbols.name(machine.symbol) == "accept")
        .unwrap()
        .symbol;
    let state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (checked.symbols.name(state.machine_symbol) == "run").then_some(state.clone())
        })
        .unwrap();
    let call = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| target_machine(&checked, call) == Some(target))
        .unwrap()
        .clone();
    let contexts = checked
        .facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(call.entry_semantic_contexts)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>();
    let path = checked
        .facts
        .contextual_cleanup
        .entry_requirements
        .paths
        .insert_many([CheckedStructuralPredicatePathSegment::Field("ready".into())]);
    let requirement = ContextualCleanupEntryRequirement {
        machine: target,
        parameter_position: 1,
        path,
        expected: true,
        antecedent: CheckedBooleanExpression::Parameter { position: 0 },
        ..Default::default()
    };
    assert_eq!(
        proves(
            &checked,
            &checked.facts,
            &state,
            &call,
            &contexts,
            &requirement
        ),
        expected
    );
}

#[test]
fn implication_call_checks_selector_and_constructor_leaf_together() {
    let source = "data Token { ready: bool; }
        machine accept(selected: bool, token: Token) {}
        machine run() { accept(false, Token { ready: false }); }";
    check_actuals(source, true);
    check_actuals(&source.replace("accept(false", "accept(true"), false);
    check_actuals(&source.replace("ready: false", "ready: true"), true);
}

#[test]
fn implication_call_uses_stored_selector_after_its_source_changes() {
    let source = "data Token { ready: bool; }
        machine accept(selected: bool, token: Token) {}
        machine run() {
            let mut current: bool = false;
            let saved: bool = current;
            current = true;
            accept(saved, Token { ready: false });
        }";
    check_actuals(source, true);
    check_actuals(&source.replace("accept(saved", "accept(current"), false);
}

#[test]
fn antecedent_transport_preserves_scalar_and_structural_namespaces() {
    let checked = checked_program(
        "data Token { ready: bool; }
        machine accept(selected: bool, token: Token) {}
        machine run(token: Token, flag: bool) { accept(!flag, token); }",
    );
    let target = checked
        .machines()
        .iter()
        .find(|machine| checked.symbols.name(machine.symbol) == "accept")
        .unwrap()
        .symbol;
    let state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (checked.symbols.name(state.machine_symbol) == "run").then_some(state)
        })
        .unwrap();
    let call = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| target_machine(&checked, call) == Some(target))
        .unwrap();
    let requirement = ContextualCleanupEntryRequirement {
        machine: target,
        antecedent: CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 1,
                path: vec![CheckedStructuralPredicatePathSegment::Field("ready".into())],
            }),
        },
        ..Default::default()
    };
    assert_eq!(
        entry_antecedent(&checked, &checked.facts, state, call, &requirement),
        Some(CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::Not(Box::new(
                CheckedBooleanExpression::Parameter { position: 0 }
            ))),
            right: Box::new(CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 0,
                path: vec![CheckedStructuralPredicatePathSegment::Field("ready".into())],
            }),
        })
    );
}

#[test]
fn implication_call_does_not_substitute_a_later_operand_write_for_the_captured_selector() {
    check_actuals(
        "data Token { ready: bool; }
        machine accept(selected: bool, token: Token) {}
        machine replacement(flag: &mut bool) -> Token ensures !flag {
            flag = false;
            Token { ready: false }
        }
        machine run() {
            let mut flag: bool = true;
            accept(flag, replacement(&mut flag));
        }",
        false,
    );
}

#[test]
fn implication_call_keeps_a_valid_selector_captured_before_a_later_operand_write() {
    check_actuals(
        "data Token { ready: bool; }
        machine accept(selected: bool, token: Token) {}
        machine replacement(flag: &mut bool) -> Token ensures flag {
            flag = true;
            Token { ready: false }
        }
        machine run() {
            let mut flag: bool = false;
            accept(flag, replacement(&mut flag));
        }",
        true,
    );
}

#[test]
fn antecedent_origin_uses_only_writes_before_its_own_operand_completion() {
    for (arguments, position, expected) in [
        (
            "flag, write(&mut flag), token",
            0,
            Some(CheckedBooleanExpression::Parameter { position: 0 }),
        ),
        ("write(&mut flag), flag, token", 1, None),
    ] {
        let checked = checked_program(&format!(
            "data Token {{ ready: bool; }}
            machine write(flag: &mut bool) -> bool {{ flag = true; true }}
            machine accept(first: bool, second: bool, token: Token) {{}}
            machine run(mut flag: bool, token: Token) {{ accept({arguments}); }}"
        ));
        let target = checked
            .machines()
            .iter()
            .find(|machine| checked.symbols.name(machine.symbol) == "accept")
            .unwrap()
            .symbol;
        let state = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, state)| {
                (checked.symbols.name(state.machine_symbol) == "run").then_some(state)
            })
            .unwrap();
        let call = checked
            .facts
            .flow
            .control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .find(|call| target_machine(&checked, call) == Some(target))
            .unwrap();
        let requirement = ContextualCleanupEntryRequirement {
            machine: target,
            antecedent: CheckedBooleanExpression::Parameter { position },
            ..Default::default()
        };
        assert_eq!(
            entry_antecedent(&checked, &checked.facts, state, call, &requirement),
            expected,
            "{arguments}"
        );
        let mut substituted = call.clone();
        substituted.operands = arena::HandleSpan::empty();
        assert!(
            entry_antecedent(&checked, &checked.facts, state, &substituted, &requirement).is_none(),
            "missing evaluated operands cannot manufacture entry custody"
        );
    }
}
