//! Transitive scalar helpers retain complete ordered bodies, storage and custody.

use super::*;

#[test]
fn scalar_array_local_control_keeps_prefix_effects_and_call_result_storage() {
    let source =
        include_str!("../../../../../../tests/omega/pass/collections/array_local_control/main.omg");
    for enabled in [true, false] {
        assert_array(
            source,
            &[TerminalScalarValue::Boolean(enabled), byte(42)],
            &[byte(if enabled { 42 } else { 13 }), byte(9)],
        );
    }
}

#[test]
fn scalar_array_control_rejects_substituted_tail_and_local_custody() {
    let original = checked_source(
        "machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine selected(enabled: bool, value: u8) -> u8 {
             let row: [u8; 2] = [7u8, 9u8];
             transition enabled { true -> (answer(row, value)) false -> 0u8 }
         }",
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected").unwrap();
    for mutation in [
        "missing control",
        "guard coordinate",
        "swapped arms",
        "return role",
        "result type",
        "omitted constructor",
        "mutable local",
        "local symbol",
        "duplicate root",
    ] {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.scalar_control.is_some())
            .unwrap();
        let state = plan.state;
        let control = plan.scalar_control.as_mut().unwrap();
        let checked_trees::CheckedScalarStateTerminator::Conditional {
            guard_statement_ordinal,
            when_true,
            when_false,
        } = &mut control.terminator
        else {
            panic!("conditional fixture");
        };
        match mutation {
            "missing control" => plan.scalar_control = None,
            "guard coordinate" => *guard_statement_ordinal = 0,
            "swapped arms" => std::mem::swap(when_true, when_false),
            "return role" => {
                let checked_trees::CheckedScalarBranchDestination::Return {
                    is_continuation, ..
                } = when_true
                else {
                    panic!("return fixture");
                };
                *is_continuation = true;
            }
            "result type" => control.primitive_type = PrimitiveType::U64,
            "omitted constructor" => {
                plan.operations.remove(0);
            }
            "mutable local" => {
                let span = changed
                    .machines()
                    .iter()
                    .flat_map(|machine| changed.machine_states(machine))
                    .find(|source| source.symbol == state)
                    .unwrap()
                    .statement_nodes;
                let StatementNode::LocalData(local) =
                    &mut changed.typed.statement_table.statements_mut(span)[0]
                else {
                    panic!("array local");
                };
                local.is_mutable = true;
            }
            "local symbol" => {
                let handle = changed.facts.values.scalar_computations.structural_arguments.iter()
                    .find_map(|(handle, argument)| match argument {
                        checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)
                            if matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ArrayLocal { .. }) => Some(handle),
                        _ => None,
                    }).unwrap();
                let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
                    changed
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .get_mut(handle)
                else {
                    panic!("local argument");
                };
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ArrayLocal {
                        symbol: symbols::SymbolHandle::invalid(),
                    };
            }
            "duplicate root" => {
                let root = changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .find_map(|(_, root)| {
                        (root.state == state && root.role == CheckedScalarExpressionRole::Return)
                            .then(|| root.clone())
                    })
                    .unwrap();
                changed.facts.values.scalar_computations.roots.append(root);
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn scalar_array_locals_preserve_conditional_call_continuations() {
    for binding in [false, true] {
        let (prefix, argument) = if binding {
            ("let row: [u8; 2] = [7u8, 9u8];", "row")
        } else {
            ("", "[7u8, 9u8]")
        };
        let source = format!(
            "machine answer(row: [u8; 2], value: u8) -> u8 {{ value }}
             machine selected(enabled: bool, value: u8) -> u8 {{
                 {prefix}
                 transition enabled {{
                     true -> (answer({argument}, value))
                     false -> 0u8
                 }}
             }}"
        );
        for (enabled, expected) in [(true, 42), (false, 0)] {
            assert_eq!(
                execute(&source, &[TerminalScalarValue::Boolean(enabled), byte(42)]),
                TerminalExecutionResult::Scalar(byte(expected)),
                "local binding: {binding}, enabled: {enabled}"
            );
        }
    }
}

#[test]
fn scalar_helpers_retain_ordered_array_bodies_across_transitive_calls() {
    assert_array(
        "machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine helper(value: u8) -> u8 {
             let row: [u8; 2] = [7u8, 9u8];
             answer(row, value)
         }
         machine forward(value: u8) -> u8 { helper(value) }
         machine selected(value: u8) -> [u8; 2] { [forward(value), 9u8] }",
        &[byte(42)],
        &[byte(42), byte(9)],
    );
}

#[test]
fn ordered_scalar_helpers_are_reachable_from_scalar_and_unit_entries() {
    let helpers = "machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine helper(value: u8) -> u8 {
             let row: [u8; 2] = [7u8, 9u8];
             answer(row, value)
         }
         machine forward(value: u8) -> u8 { helper(value) }";
    assert_eq!(
        execute(
            &format!("{helpers} machine selected(value: u8) -> u8 {{ forward(value) }}"),
            &[byte(42)],
        ),
        TerminalExecutionResult::Scalar(byte(42)),
    );
    assert_eq!(
        execute(
            &format!(
                "{helpers}
                 machine consume(value: u8) {{}}
                 machine selected(value: u8) {{ consume(forward(value)); }}"
            ),
            &[byte(42)],
        ),
        TerminalExecutionResult::Unit,
    );
}

#[test]
fn ordered_scalar_helpers_retain_unit_structural_and_scalar_dependencies() {
    for (array_type, literal) in [("[u8; 2]", "[7u8, 9u8]"), ("[u8; 0]", "[]")] {
        assert_array(
            &format!(
                "machine touch() {{}}
                 machine keep(row: {array_type}) -> {array_type} {{ row }}
                 machine answer(row: {array_type}, value: u8) -> u8 {{ value }}
                 machine inner(value: u8) -> u8 {{
                     touch();
                     let row: {array_type} = keep({literal});
                     answer(row, value)
                 }}
                 machine helper(value: u8) -> u8 {{
                     touch();
                     let row: {array_type} = keep({literal});
                     let nested: u8 = inner(value);
                     answer(row, nested)
                 }}
                 machine selected(value: u8) -> [u8; 2] {{ [helper(value), inner(9u8)] }}"
            ),
            &[byte(42)],
            &[byte(42), byte(9)],
        );
    }
}

#[test]
fn ordered_helpers_and_scalar_graph_unit_calls_share_one_catalog() {
    let source = "machine replace(destination: &mut bool) { destination = true; }
         machine branch(value: u8) -> u8 {
             let mut enabled: bool = false;
             replace(&mut enabled);
             transition enabled { true -> value false -> 0u8 }
         }
         machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine helper(value: u8) -> u8 {
             let row: [u8; 2] = [branch(value), 9u8];
             answer(row, value)
         }
         machine gate(value: u8) -> u8 {
             let mut enabled: bool = false;
             replace(&mut enabled);
             transition enabled { true -> (helper(value)) false -> 0u8 }
         }
         machine selected(value: u8) -> [u8; 2] { [gate(value), 9u8] }";
    let checked = checked_source(source);
    for name in ["branch", "gate"] {
        let symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap()
            .symbol;
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(symbol)
            .unwrap_or_else(|| panic!("mixed closure retains scalar graph owner {name}"));
        assert!(
            graph
                .states
                .iter()
                .any(|state| !state.unit_operations.is_empty())
        );
    }
    assert_array(source, &[byte(42)], &[byte(42), byte(9)]);
}

#[test]
fn ordered_scalar_helper_stores_keep_order_across_fuel_resumption() {
    assert_array(
        &format!(
            "{WRITE}
             machine touch() {{}}
             machine answer(row: [u8; 2], value: u8) -> u8 {{ value }}
             machine helper(value: u8) -> u8 {{
                 let mut current: u8 = 0;
                 let row: [u8; 2] = [write(&mut current, 7u8), write(&mut current, value)];
                 touch();
                 answer(row, current)
             }}
             machine selected(value: u8) -> [u8; 2] {{ [helper(value), helper(9u8)] }}"
        ),
        &[byte(42)],
        &[byte(42), byte(9)],
    );
}

#[test]
fn transitive_ordered_scalar_body_rejects_source_contract_and_result_corruption() {
    let original = checked_source(
        "machine touch() {}
         machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine helper(value: u8) -> u8 {
             touch();
             let row: [u8; 2] = [7u8, 9u8];
             answer(row, value)
         }
         machine selected(value: u8) -> [u8; 1] { [helper(value)] }",
    );
    let helper = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "helper")
        .expect("source helper")
        .symbol;
    assert!(
        original
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(helper)
            .is_none()
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("real transitive operation body lowers before corruption");
    for mutation in [
        "missing body",
        "duplicate body",
        "missing result",
        "result type",
        "result binding",
        "source statement",
        "contract fingerprint",
        "contract commitment",
        "omitted statement",
    ] {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        let position = plans
            .iter()
            .position(|plan| plan.machine == helper)
            .unwrap();
        match mutation {
            "missing body" => {
                plans.remove(position);
            }
            "duplicate body" => plans.push(plans[position].clone()),
            "missing result" => plans[position].scalar_result = None,
            "result type" => {
                plans[position]
                    .scalar_result
                    .as_mut()
                    .unwrap()
                    .primitive_type = PrimitiveType::Bool
            }
            "result binding" => {
                plans[position]
                    .scalar_result
                    .as_mut()
                    .unwrap()
                    .binding_ordinal = u32::MAX
            }
            "source statement" => {
                plans[position]
                    .scalar_result
                    .as_mut()
                    .unwrap()
                    .statement_index = u32::MAX
            }
            "contract fingerprint" => plans[position].contract_report_fingerprint = 0,
            "contract commitment" => {
                plans[position].contract_commitment =
                    checked_trees::MachineContractCommitment::from_digest([0; 32])
            }
            "omitted statement" => {
                plans[position].operations.remove(0);
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}
