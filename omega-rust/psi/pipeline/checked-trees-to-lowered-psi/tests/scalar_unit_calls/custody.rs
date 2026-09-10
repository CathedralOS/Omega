//! Cached call rows cannot replace authored order, access, or source referents.

use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

fn original() -> (checked_trees::CheckedTrees, symbols::SymbolHandle) {
    let checked = super::checked(super::BOOLEAN_BRANCH);
    let _ = terminal_production::produce_terminal_artifact(&checked, "observe")
        .expect("unmodified source must publish before mutation");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap()
        .symbol;
    (checked, machine)
}

#[test]
fn scalar_unit_call_rejects_missing_duplicate_reordered_or_substituted_rows() {
    let (original, machine) = original();
    for mutation in 0..8 {
        let mut changed = original.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == machine)
            .unwrap();
        let operations = &mut graph.states[0].unit_operations;
        assert_eq!(operations.len(), 1);
        match mutation {
            0 => operations.clear(),
            1 => operations.push(operations[0].clone()),
            _ => {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    ..
                } = &mut operations[0]
                else {
                    panic!("Unit call")
                };
                match mutation {
                    2 => coordinate.statement_index = 0,
                    3 => coordinate.call_ordinal += 1,
                    4 => *target_machine = machine,
                    5 => structural_arguments[0].access = CheckedStructuralAccess::SharedBorrow,
                    6 => {
                        structural_arguments[0].source =
                            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index: 0,
                            }
                    }
                    7 => {
                        scalar_arguments[0] = checked_trees::CheckedCallScalarArgument::Pure(
                            checked_trees::CheckedScalarExpression::Boolean(Box::new(
                                checked_trees::CheckedBooleanExpression::Parameter { position: 0 },
                            )),
                        )
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&changed, "observe").is_err(),
            "accepted Unit call custody mutation {mutation}"
        );
    }
}

#[test]
fn scalar_unit_call_requires_its_exact_borrow_occurrence() {
    let (original, machine) = original();
    let state = original
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == machine)
        .unwrap()
        .1;
    let call = original
        .facts
        .borrow
        .calls
        .span_or_empty(state.calls)
        .first()
        .unwrap();
    let handle = original
        .facts
        .borrow
        .calls
        .iter()
        .find(|(_, candidate)| std::ptr::eq(*candidate, call))
        .unwrap()
        .0;
    for mutation in 0..5 {
        let mut changed = original.clone();
        let call = changed.facts.borrow.calls.get_mut(handle);
        match mutation {
            0 => call.accesses = arena::HandleSpan::empty(),
            1 => call.statement_index = 0,
            2 => call.target_symbol = machine,
            3 => call.has_receiver = true,
            4 => call.receiver_symbol = machine,
            _ => unreachable!(),
        }
        assert!(
            terminal_production::produce_terminal_artifact(&changed, "observe").is_err(),
            "accepted missing or substituted borrow occurrence {mutation}"
        );
    }
}

#[test]
fn coherent_call_and_borrow_substitution_cannot_select_another_local() {
    let source = super::BOOLEAN_BRANCH.replace(
        "    replace(&mut scratch, replacement);",
        "    let mut spare: bool = replacement;\n    replace(&mut spare, initial);\n    replace(&mut scratch, replacement);",
    );
    let mut changed = super::checked(&source);
    let _ = terminal_production::produce_terminal_artifact(&changed, "observe").unwrap();
    let machine = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap()
        .symbol;
    let graph = changed
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter_mut()
        .find(|graph| graph.machine == machine)
        .unwrap();
    let locals = &graph.states[0].primitive_locals;
    assert_eq!(locals.len(), 2);
    let scratch = locals[0].symbol;
    let spare = locals[1].symbol;
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &mut graph.states[0].unit_operations[1]
    else {
        panic!("second Unit call")
    };
    assert_eq!(
        structural_arguments[0].source,
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: scratch }
    );
    structural_arguments[0].source =
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: spare };
    let accesses = changed
        .facts
        .borrow
        .argument_accesses
        .iter()
        .filter_map(|(handle, access)| (access.root_symbol == scratch).then_some(handle))
        .collect::<Vec<_>>();
    assert!(!accesses.is_empty());
    for handle in accesses {
        changed
            .facts
            .borrow
            .argument_accesses
            .get_mut(handle)
            .root_symbol = spare;
    }
    assert!(
        terminal_production::produce_terminal_artifact(&changed, "observe").is_err(),
        "matching cached call/borrow rows cannot replace the authored local"
    );
}

#[test]
fn unit_call_preserves_the_order_of_distinct_borrow_occurrences() {
    let source = r#"
machine replace_pair(left: &mut bool, right: &mut bool, replacement: bool) {
    left = replacement;
    right = replacement;
}
machine observe(initial: bool, replacement: bool) -> u64 {
    let mut scratch: bool = initial;
    let mut spare: bool = initial;
    replace_pair(&mut scratch, &mut spare, replacement);
    transition scratch { true -> 1 false -> 0 }
}
"#;
    let mut changed = super::checked(source);
    let _ = terminal_production::produce_terminal_artifact(&changed, "observe")
        .expect("two distinct primitive borrows publish before corruption");
    let machine = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap()
        .symbol;
    let state = changed
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == machine)
        .unwrap()
        .1;
    let call = changed
        .facts
        .borrow
        .calls
        .span_or_empty(state.calls)
        .first()
        .unwrap();
    let accesses = changed
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses);
    assert_eq!(
        accesses.len(),
        3,
        "two borrows followed by the scalar argument read"
    );
    let first = accesses[0].clone();
    let second = accesses[1].clone();
    assert_ne!(first.root_symbol, second.root_symbol);
    let handles = accesses
        .iter()
        .map(|access| {
            changed
                .facts
                .borrow
                .argument_accesses
                .iter()
                .find(|(_, candidate)| std::ptr::eq(*candidate, access))
                .unwrap()
                .0
        })
        .collect::<Vec<_>>();
    *changed.facts.borrow.argument_accesses.get_mut(handles[0]) = second;
    *changed.facts.borrow.argument_accesses.get_mut(handles[1]) = first;
    assert!(
        terminal_production::produce_terminal_artifact(&changed, "observe").is_err(),
        "reordered borrow occurrences must not replace authored argument order"
    );
}
