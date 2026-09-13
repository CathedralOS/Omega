use super::*;

fn checked() -> CheckedTrees {
    let source = "data Tag { case First; case Second; }
        machine choose(selector: u64) -> Tag {
            match selector { 0 -> Tag::First, _ -> Tag::Second }
        }";
    checked_source(source)
}

fn checked_source(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

#[test]
fn moved_record_replay_preserves_value_origin_and_requires_exact_transfer() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let original = checked_source(
        "data Owned { value: u64; }
         machine moved() -> u64 {
             let keep: Owned = Owned { value: 17 };
             let first: Owned = Owned { value: 256 };
             let second: Owned = first;
             let third: Owned = second;
             keep.value ^ third.value
         }",
    );
    let mut moves = Vec::new();
    crate::lower_machine(&original, "moved")
        .expect("chained moves retain the survivor and dispose only their current owners");
    for (_, root) in original.facts.values.structural_values.roots.iter() {
        let operation = CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: root.statement_ordinal,
                binding_ordinal: root.statement_ordinal,
                type_identity: original
                    .normalized_type_identity(root.type_reference)
                    .into_string(),
                multiplicity: original.type_multiplicity(root.type_reference),
            },
            value: root.root,
            calls: Vec::new(),
            discard_result_on_return: false,
        };
        validate(&original, root.machine, root.state, &operation)
            .expect("each moved local preserves its original establishment");
        if root.statement_ordinal >= 2 {
            moves.push((root.machine, root.state, root.statement_ordinal, operation));
        }
    }
    assert_eq!(moves.len(), 2);
    for (machine, state, ordinal, operation) in moves {
        for change_destination in [true, false] {
            let mut forged = original.clone();
            let mut changed = false;
            let permissions = forged
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            for handle in permissions {
                let event = forged.facts.flow.ownership.permissions.get_mut(handle);
                if event.machine_symbol != machine
                    || event.state_symbol != state
                    || event.source
                        != (PermissionEventSource::Statement {
                            statement_index: ordinal as usize,
                        })
                {
                    continue;
                }
                if change_destination && event.kind == PermissionEventKind::Establish {
                    event.provenance = PermissionProvenance::Established {
                        machine_symbol: machine,
                        state_symbol: state,
                        source: event.source,
                    };
                    changed = true;
                } else if !change_destination && event.kind == PermissionEventKind::Transfer {
                    event.kind = PermissionEventKind::AffineDrop;
                    changed = true;
                }
            }
            assert!(changed);
            assert!(
                validate(&forged, machine, state, &operation).is_err(),
                "a moved result cannot mint provenance or replace its source transfer with disposal"
            );
        }
    }
}

#[test]
fn record_replay_rejects_swapped_same_carrier_fields_and_operands() {
    let original = checked_source(
        "data Pair[copy] { first: u64; second: u64; }
        machine make(first: u64, second: u64) -> Pair {
            Pair { second: second, first: first }
        }",
    );
    let (machine, state, operation) = operation(&original);
    validate(&original, machine, state, &operation).expect("authored record");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("construction");
    };
    let CheckedStructuralValueKind::Record { fields, .. } = original
        .facts
        .values
        .structural_values
        .nodes
        .get(value)
        .kind
    else {
        panic!("record");
    };
    let first = fields.start();
    let second = arena::Handle::from_parts(first.arena_index() + 1, first.generation());
    for exchange_field_identity in [false, true] {
        let mut forged = original.clone();
        let left = forged
            .facts
            .values
            .structural_values
            .record_fields
            .get(first)
            .clone();
        let right = forged
            .facts
            .values
            .structural_values
            .record_fields
            .get(second)
            .clone();
        if exchange_field_identity {
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(first)
                .field = right.field;
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(second)
                .field = left.field;
        } else {
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(first)
                .value = right.value;
            forged
                .facts
                .values
                .structural_values
                .record_fields
                .get_mut(second)
                .value = left.value;
        }
        assert!(validate(&forged, machine, state, &operation).is_err());
    }
}

fn operation(
    checked: &CheckedTrees,
) -> (SymbolHandle, SymbolHandle, CheckedUnitEffectOperationPlan) {
    let (_, root) = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .next()
        .expect("fresh value root");
    (
        root.machine,
        root.state,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: root.statement_ordinal,
                binding_ordinal: 0,
                type_identity: checked
                    .normalized_type_identity(root.type_reference)
                    .into_string(),
                multiplicity: checked.type_multiplicity(root.type_reference),
            },
            value: root.root,
            calls: Vec::new(),
            discard_result_on_return: false,
        },
    )
}

#[test]
fn structural_replay_rejects_substituted_integer_pattern_payload() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).unwrap();
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("structural value");
    };
    let CheckedStructuralValueKind::Dispatch { arms, .. } =
        checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("dispatch");
    };
    let CheckedScalarDispatchPattern::Value(pattern) = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span(arms)
        .unwrap()[0]
        .pattern
    else {
        panic!("value pattern");
    };
    let checked_trees::CheckedScalarComputationKind::Value(
        checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
    ) = &mut checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(pattern)
        .kind
    else {
        panic!("retained integer pattern");
    };
    *literal =
        numerics::literals::IntegerLiteral::from_value(17).with_landing(literal.landing().unwrap());
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn structural_replay_rejects_swapped_case_results_and_missing_coverage() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).expect("original source correspondence");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("value operation");
    };
    let CheckedStructuralValueKind::Dispatch { arms, .. } =
        checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("match");
    };
    let original = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span(arms)
        .unwrap()
        .to_vec();
    let retained = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span_mut(arms)
        .unwrap();
    retained[0].value = original[1].value;
    retained[1].value = original[0].value;
    assert!(validate(&checked, machine, state, &operation).is_err());
    checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span_mut(arms)
        .unwrap()
        .clone_from_slice(&original);
    let CheckedStructuralValueKind::Dispatch { arms, .. } = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(value)
        .kind
    else {
        panic!("match");
    };
    *arms = arena::HandleSpan::from_parts(arms.start(), 1);
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn structural_replay_rejects_case_identity_and_root_owner_substitution() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).expect("original source correspondence");
    let case = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(node.kind, CheckedStructuralValueKind::Case(_)).then_some(handle)
        })
        .unwrap();
    let original = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(case)
        .clone();
    let CheckedStructuralValueKind::Case(construction) = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(case)
        .kind
    else {
        panic!("case");
    };
    construction.case = machine;
    assert!(validate(&checked, machine, state, &operation).is_err());
    *checked.facts.values.structural_values.nodes.get_mut(case) = original;
    let root = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .next()
        .unwrap()
        .0;
    checked
        .facts
        .values
        .structural_values
        .roots
        .get_mut(root)
        .machine = state;
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn nested_call_replay_rejects_an_equal_row_outside_the_owning_state() {
    let mut checked = checked_source(
        "data Inner { value: u64; } data Outer { child: Inner; }
        machine make_child(value: u64) -> Inner { Inner { value: value } }
        machine make() -> Outer { Outer { child: make_child(7) } }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make")
        .unwrap();
    let symbol = machine.symbol;
    let state = checked.machine_states(machine)[0].symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .expect("ordinary nested constructor");
    let operation = plan
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            )
        })
        .unwrap()
        .clone();
    validate(&checked, symbol, state, &operation).expect("actual nested call occurrence");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } = &operation else {
        unreachable!()
    };
    let [call] = calls.as_slice() else {
        panic!("one nested call")
    };
    let handle = call.value;
    let CheckedStructuralValueKind::Call { source_call } = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(handle)
        .kind
    else {
        panic!("call value")
    };
    let equal_row = checked.facts.flow.control.calls.get(source_call).clone();
    let outside_state = checked.facts.flow.control.calls.append(equal_row);
    checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(handle)
        .kind = CheckedStructuralValueKind::Call {
        source_call: outside_state,
    };
    assert!(
        validate(&checked, symbol, state, &operation)
            .unwrap_err()
            .to_string()
            .contains("outside its source state")
    );
}

#[test]
fn owned_record_child_replay_rejects_same_carrier_parameter_substitution() {
    let mut checked = checked_source(
        "data Inner { value: u64; } data Outer { child: Inner; }
        machine wrap(first: Inner, second: Inner) -> Outer { Outer { child: first } }",
    );
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).unwrap();
    let value = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, value)| {
            matches!(value.kind, CheckedStructuralValueKind::Place(_)).then_some(handle)
        })
        .unwrap();
    let CheckedStructuralValueKind::Place(argument) = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(value)
        .kind
    else {
        unreachable!()
    };
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert!(
        validate(&checked, machine, state, &operation)
            .unwrap_err()
            .to_string()
            .contains("substituted its parameter")
    );
}

#[test]
fn whole_record_root_replay_rejects_same_carrier_source_substitution() {
    let mut checked = checked_source(
        "data Value [copy] { value: u64; } machine copy(first: Value, second: Value) -> Value { first }",
    );
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).expect("whole record source");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("whole record binding");
    };
    let CheckedStructuralValueKind::Place(argument) = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(value)
        .kind
    else {
        panic!("whole record operand");
    };
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert!(
        validate(&checked, machine, state, &operation)
            .unwrap_err()
            .to_string()
            .contains("substituted its parameter")
    );
}
