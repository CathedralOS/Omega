//! Literal operands retain call/formal custody and the shared evaluation order.

use super::*;
use checked_trees::CheckedArrayConstructionSource;

fn byte(value: u8) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: IntegerValue::Unsigned(u128::from(value)),
    }
}

fn execute(source: &str, arguments: &[TerminalScalarValue]) -> TerminalExecutionResult {
    let checked = checked_source(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
        .unwrap_or_else(|error| panic!("literal argument source must lower: {error:?}\n{source}"));
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    assert_eq!(
        terminal_codec::decode_module(&semantic).unwrap(),
        lowered.semantic_module
    );
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), arguments)
        .unwrap_or_else(|error| {
            panic!("decoded literal arguments must execute: {error:?}\n{source}")
        })
}

fn assert_array(source: &str, arguments: &[TerminalScalarValue], expected: &[TerminalScalarValue]) {
    let TerminalExecutionResult::ScalarArray(result) = execute(source, arguments) else {
        panic!("expected whole array result for {source}");
    };
    assert_eq!(result.value.elements, expected, "{source}");
}

#[test]
fn literal_array_arguments_compose_unit_scalar_and_structural_calls() {
    assert_eq!(
        execute(
            "machine consume(row: [u8; 2]) {}
         machine selected(value: u8) { consume([value, 7u8 + 2u8]); }",
            &[byte(42)],
        ),
        TerminalExecutionResult::Unit
    );
    assert_array(
        "machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine selected() -> [u8; 2] {
             let result: u8 = answer([7, 9], 42u8);
             [result, 9]
         }",
        &[],
        &[byte(42), byte(9)],
    );
    for completion in [
        "keep([value, 7u8 + 2u8])",
        "keep(keep([value, 7u8 + 2u8]))",
        "let row: [u8; 2] = keep([value, 7u8 + 2u8]); keep(row)",
    ] {
        assert_array(
            &format!(
                "machine keep(row: [u8; 2]) -> [u8; 2] {{ row }}
             machine selected(value: u8) -> [u8; 2] {{ {completion} }}"
            ),
            &[byte(42)],
            &[byte(42), byte(9)],
        );
    }
}

#[test]
fn literal_array_argument_supports_direct_scalar_completion() {
    assert_eq!(
        execute(
            "machine answer(row: [u8; 2], value: u8) -> u8 { value }
             machine selected() -> u8 { answer([7, 9], 42u8) }",
            &[],
        ),
        TerminalExecutionResult::Scalar(byte(42)),
    );
}

#[test]
fn literal_array_scalar_graph_rejects_result_drift_without_closure_fallback() {
    let original = checked_source(
        "machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine selected() -> u8 { answer([7, 9], 42u8) }",
    );
    let selected = checked_trees_to_lowered_psi::select_terminal_machine(&original, "selected")
        .unwrap()
        .machine;
    assert!(
        original
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(selected)
            .is_some(),
        "the overlapping operation-body plan must not rescue an invalid graph"
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected").unwrap();
    for mutation in ["type", "binding", "statement"] {
        let mut changed = original.clone();
        let state = &mut changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == selected)
            .unwrap()
            .states[0];
        match mutation {
            "type" => state.result_type = PrimitiveType::Bool,
            "binding" => state.bindings[0].statement_ordinal = u32::MAX,
            "statement" => {
                state.terminator = checked_trees::CheckedScalarStateTerminator::Return {
                    statement_ordinal: u32::MAX,
                }
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn literal_array_scalar_completion_rejects_forged_result_metadata() {
    // A whole array local belongs to the structural operation body. Retain
    // that semantic owner while corrupting its scalar completion metadata.
    let original = checked_source(
        "machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine selected() -> u8 { let row: [u8; 2] = [7, 9]; answer(row, 42u8) }",
    );
    let selected = checked_trees_to_lowered_psi::select_terminal_machine(&original, "selected")
        .unwrap()
        .machine;
    assert!(
        original
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(selected)
            .is_none(),
        "the operation-body result is selected, not a redundant graph plan"
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("scalar completion lowers before metadata corruption");
    for mutation in [
        "type",
        "binding",
        "statement",
        "deleted result",
        "dual result",
    ] {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.scalar_result.is_some()
                    && plan.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                        )
                    })
            })
            .expect("scalar completion with literal constructor");
        match mutation {
            "type" => {
                plan.scalar_result.as_mut().unwrap().primitive_type =
                    checked_trees::types::PrimitiveType::Bool
            }
            "binding" => plan.scalar_result.as_mut().unwrap().binding_ordinal = u32::MAX,
            "statement" => plan.scalar_result.as_mut().unwrap().statement_index = u32::MAX,
            "deleted result" => plan.scalar_result = None,
            "dual result" => {
                let binding = plan
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. } => {
                            Some(result.clone())
                        }
                        _ => None,
                    })
                    .expect("array argument binding");
                plan.structural_result = Some(binding.into());
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn literal_array_arguments_follow_scalar_local_and_selected_boolean_prefixes() {
    assert_array(
        "machine identity(value: u8) -> u8 { value }
         machine keep(row: [u8; 2]) -> [u8; 2] { row }
         machine selected(value: u8) -> [u8; 2] {
             let prefix: u8 = identity(value);
             keep([prefix, 7u8 + 2u8])
         }",
        &[byte(42)],
        &[byte(42), byte(9)],
    );
    for enabled in [false, true] {
        assert_array(
            "machine identity(value: bool) -> bool { value }
             machine pick(prefix: bool, row: [bool; 2]) -> [bool; 2] { row }
             machine selected(enabled: bool) -> [bool; 2] {
                 let prefix: bool = enabled || identity(false);
                 pick(prefix && identity(true), [identity(prefix), !identity(prefix)])
             }",
            &[TerminalScalarValue::Boolean(enabled)],
            &[
                TerminalScalarValue::Boolean(enabled),
                TerminalScalarValue::Boolean(!enabled),
            ],
        );
    }
}

#[test]
fn literal_array_arguments_keep_formal_order_and_nested_empty_shapes() {
    for selected in ["first", "second"] {
        assert_array(&format!(
            "machine identity(value: u8) -> u8 {{ value }}
             machine pick(prefix: u8, first: [u8; 2], middle: u8, second: [u8; 2]) -> [u8; 2] {{ {selected} }}
             machine selected() -> [u8; 2] {{ pick(identity(3u8), [7, 9], identity(4u8), [42, 9]) }}"
        ), &[], &[byte(if selected == "first" {7} else {42}), byte(9)]);
    }
    for (array_type, value, expected) in [
        (
            "[[u8; 2]; 2]",
            "[[7, 9], [11, 13]]",
            vec![byte(7), byte(9), byte(11), byte(13)],
        ),
        ("[u8; 0]", "[]", vec![]),
        ("[[u8; 0]; 1]", "[[]]", vec![]),
        ("[[u8; 2]; 0]", "[]", vec![]),
    ] {
        assert_array(
            &format!(
                "machine keep(row: {array_type}) -> {array_type} {{ row }}
             machine selected() -> {array_type} {{ keep(keep({value})) }}"
            ),
            &[],
            &expected,
        );
    }
}

const MUTATION_HELPER: &str =
    "machine write(destination: &mut u8, value: u8) -> u8 { destination = value; value }";

#[test]
fn literal_array_arguments_compose_selected_match_locals_and_leaves() {
    for enabled in [false, true] {
        let local = if enabled { 1 } else { 2 };
        let leaf = if enabled { 3 } else { 4 };
        assert_array(
            &format!(
                "{MUTATION_HELPER}
                 machine keep(row: [u8; 4]) -> [u8; 4] {{ row }}
                 machine selected(enabled: bool) -> [u8; 4] {{
                     let mut value: u8 = 0;
                     let prefix: u8 = match enabled {{
                         true -> write(&mut value, 1u8),
                         false -> write(&mut value, 2u8)
                     }};
                     keep([prefix, value, match enabled {{
                         true -> write(&mut value, 3u8),
                         false -> write(&mut value, 4u8)
                     }}, value])
                 }}"
            ),
            &[TerminalScalarValue::Boolean(enabled)],
            &[byte(local), byte(local), byte(leaf), byte(leaf)],
        );
    }
}

#[test]
fn literal_array_construction_interleaves_with_mutating_scalar_actuals() {
    for (returned, expected) in [("first", 1), ("second", 2)] {
        assert_array(&format!(
            "{MUTATION_HELPER}
             machine pick(prefix: u8, first: [u8; 1], middle: u8, second: [u8; 1], suffix: u8) -> [u8; 1] {{ {returned} }}
             machine selected() -> [u8; 1] {{
                 let mut value: u8 = 0;
                 pick(write(&mut value, 1u8), [value], write(&mut value, 2u8), [value], write(&mut value, 3u8))
             }}"
        ), &[], &[byte(expected)]);
    }
    assert_array(
        &format!(
            "{MUTATION_HELPER}
         machine keep(row: [u8; 4]) -> [u8; 4] {{ row }}
         machine selected() -> [u8; 4] {{
             let mut value: u8 = 0;
             keep([write(&mut value, 1u8), value, write(&mut value, 2u8), value])
         }}"
        ),
        &[],
        &[byte(1), byte(1), byte(2), byte(2)],
    );
}

#[test]
fn literal_array_boolean_leaves_keep_short_circuit_selection_and_order() {
    for (left, right, expected) in [("false", "true", 0), ("true", "false", 2)] {
        assert_array(&format!(
            "machine flag(destination: &mut u8, value: u8) -> bool {{ destination = value; true }}
             machine keep(row: [bool; 2]) -> [bool; 2] {{ row }}
             machine selected() -> [u8; 1] {{
                 let mut value: u8 = 0;
                 let flags: [bool; 2] = keep([{left} && flag(&mut value, 1u8), {right} || flag(&mut value, 2u8)]);
                 [value]
             }}"
        ), &[], &[byte(expected)]);
    }
    assert_array(
        "machine identity(value: bool) -> bool { value }
         machine keep(row: [bool; 2]) -> [bool; 2] { row }
         machine selected() -> [bool; 2] { keep([identity(true), !identity(true)]) }",
        &[],
        &[
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ],
    );
}

fn custody_fixture(empty: bool) -> CheckedTrees {
    let (array_type, left, right) = if empty {
        ("[u8; 0]", "[]", "[]")
    } else {
        (
            "[u8; 2]",
            "[value, identity(value)]",
            "[value, identity(value)]",
        )
    };
    checked_source(&format!(
        "machine identity(value: u8) -> u8 {{ value }}
         machine pick(prefix: u8, first: {array_type}, middle: u8, second: {array_type}) -> {array_type} {{ second }}
         machine selected(value: u8) -> {array_type} {{ pick(3u8, {left}, 4u8, {right}) }}"
    ))
}

#[test]
fn computed_scalar_locals_keep_short_circuit_effects_before_ordinary_calls() {
    for enabled in [false, true] {
        for consumer in [
            "consume(evaluated);",
            "let flags: [bool; 2] = keep([evaluated, !evaluated]);",
        ] {
            assert_array(
                &format!(
                    "machine flag(destination: &mut u8) -> bool {{ destination = 7u8; true }}
                     machine consume(value: bool) {{}}
                     machine keep(row: [bool; 2]) -> [bool; 2] {{ row }}
                     machine selected(enabled: bool) -> [u8; 1] {{
                         let mut value: u8 = 0;
                         let evaluated: bool = enabled && flag(&mut value);
                         {consumer}
                         [value]
                     }}"
                ),
                &[TerminalScalarValue::Boolean(enabled)],
                &[byte(if enabled { 7 } else { 0 })],
            );
        }
    }
}

#[test]
fn pure_short_circuit_local_precedes_ordinary_and_literal_array_calls() {
    for enabled in [false, true] {
        for consumer in [
            "consume(evaluated);",
            "let flags: [bool; 1] = keep([evaluated]);",
        ] {
            assert_array(
                &format!(
                    "machine consume(value: bool) {{}}
                     machine keep(row: [bool; 1]) -> [bool; 1] {{ row }}
                     machine selected(enabled: bool) -> [bool; 1] {{
                         let evaluated: bool = enabled || false;
                         {consumer}
                         [evaluated]
                     }}"
                ),
                &[TerminalScalarValue::Boolean(enabled)],
                &[TerminalScalarValue::Boolean(enabled)],
            );
        }
    }
}

#[test]
fn computed_scalar_local_rejects_substituted_handle_and_source_root() {
    let original = checked_source(
        "machine identity(value: bool) -> bool { value }
         machine keep(row: [bool; 2]) -> [bool; 2] { row }
         machine selected(enabled: bool) -> [bool; 2] {
             let first: bool = enabled && identity(true);
             let second: bool = enabled || identity(false);
             keep([first, second])
         }",
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("computed locals lower before source corruption");
    let (plan_index, plan) = original
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .enumerate()
        .find(|(_, plan)| {
            plan.operations.iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        value: checked_trees::CheckedCallScalarArgument::Computation(_),
                        ..
                    }
                )
            })
        })
        .expect("computed local owner");
    let locals = plan
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| match operation {
            CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                result,
                value: checked_trees::CheckedCallScalarArgument::Computation(handle),
            } => Some((index, result.statement_index, *handle)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        locals.len(),
        2,
        "two independent same-typed computed locals"
    );
    let root = original
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find_map(|(handle, root)| {
            (root.state == plan.state
                && root.statement_ordinal == locals[0].1
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::LocalInitializer { .. }
                ))
            .then_some(handle)
        })
        .expect("first local computation custody");
    for mutation in [
        "operation handle",
        "retained root",
        "authored root",
        "root statement",
    ] {
        let mut changed = original.clone();
        match mutation {
            "operation handle" => {
                let CheckedUnitEffectOperationPlan::EstablishScalarLocal { value, .. } =
                    &mut changed.facts.flow.terminal_unit_effects.machines[plan_index].operations
                        [locals[0].0]
                else {
                    unreachable!()
                };
                *value = checked_trees::CheckedCallScalarArgument::Computation(locals[1].2);
            }
            "retained root" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(root)
                    .root = locals[1].2
            }
            "authored root" => {
                let replacement = changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(locals[1].2)
                    .authored_root;
                changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(locals[0].2)
                    .authored_root = replacement;
            }
            "root statement" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(root)
                    .statement_ordinal = locals[1].1
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn literal_array_constructor_sources_reject_forged_call_formal_and_empty_owners() {
    for empty in [false, true] {
        let original = custody_fixture(empty);
        checked_trees_to_lowered_psi::lower_machine(&original, "selected")
            .expect("literal constructors lower before owner corruption");
        for mutation in [
            "statement source",
            "call ordinal",
            "formal position",
            "statement index",
            "deleted constructor",
            "reordered constructors",
            "swapped actual bindings",
        ] {
            let mut changed = original.clone();
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| {
                    plan.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                                source: CheckedArrayConstructionSource::CallArgument { .. },
                                ..
                            }
                        )
                    })
                })
                .expect("literal argument constructor owner");
            let constructors = plan
                .operations
                .iter()
                .enumerate()
                .filter_map(|(position, operation)| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishScalarArray {
                            source: CheckedArrayConstructionSource::CallArgument { .. },
                            ..
                        }
                    )
                    .then_some(position)
                })
                .collect::<Vec<_>>();
            assert_eq!(constructors.len(), 2);
            if mutation == "deleted constructor" {
                plan.operations.remove(constructors[0]);
            } else if mutation == "reordered constructors" {
                plan.operations.swap(constructors[0], constructors[1]);
            } else if mutation == "swapped actual bindings" {
                let arguments = plan
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralCall {
                            structural_arguments,
                            ..
                        } if structural_arguments.len() == 2 => Some(structural_arguments),
                        _ => None,
                    })
                    .expect("two same-typed literal actuals");
                arguments.swap(0, 1);
            } else {
                let CheckedUnitEffectOperationPlan::EstablishScalarArray { source, result, .. } =
                    &mut plan.operations[constructors[0]]
                else {
                    unreachable!()
                };
                match mutation {
                    "statement source" => *source = CheckedArrayConstructionSource::Statement,
                    "call ordinal" => {
                        let CheckedArrayConstructionSource::CallArgument { call_ordinal, .. } =
                            source
                        else {
                            unreachable!()
                        };
                        *call_ordinal = u32::MAX;
                    }
                    "formal position" => {
                        let CheckedArrayConstructionSource::CallArgument {
                            parameter_position, ..
                        } = source
                        else {
                            unreachable!()
                        };
                        assert_ne!(
                            *parameter_position, 3,
                            "first literal owns the first array formal"
                        );
                        *parameter_position = 3;
                    }
                    "statement index" => result.statement_index = u32::MAX,
                    _ => unreachable!(),
                }
            }
            reject(&changed, mutation);
        }
    }
}

#[test]
fn literal_array_nested_producers_cannot_impersonate_the_returned_call_result() {
    for (array_type, literal) in [("[u8; 2]", "[7, 9]"), ("[u8; 0]", "[]")] {
        let original = checked_source(&format!(
            "machine keep(row: {array_type}) -> {array_type} {{ row }}
             machine selected() -> {array_type} {{ keep(keep({literal})) }}"
        ));
        checked_trees_to_lowered_psi::lower_machine(&original, "selected")
            .expect("nested literal argument lowers before return corruption");
        for constructor in [true, false] {
            let mut changed = original.clone();
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| {
                    plan.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                                source: CheckedArrayConstructionSource::CallArgument { .. },
                                ..
                            }
                        )
                    })
                })
                .expect("literal argument owner");
            let binding_ordinal = plan
                .operations
                .iter()
                .find_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source: CheckedArrayConstructionSource::CallArgument { .. },
                        result,
                        ..
                    } if constructor => Some(result.binding_ordinal),
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        coordinate, result, ..
                    } if !constructor && coordinate.call_ordinal != 0 => {
                        Some(result.binding_ordinal)
                    }
                    _ => None,
                })
                .expect("same-statement operand producer");
            plan.structural_result
                .as_mut()
                .expect("outer call result")
                .source =
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                };
            reject(
                &changed,
                "argument producer substituted for enclosing call return",
            );
        }
    }
}

#[test]
fn literal_array_argument_payload_cannot_change_under_retained_element_facts() {
    let original = checked_source(
        "machine keep(row: [u8; 2]) -> [u8; 2] { row }
         machine selected() -> [u8; 2] { keep([7, 9]) }",
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("literal argument payload lowers before corruption");
    let expression = original
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .flat_map(|machine| &machine.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                source_site: Some(checked_trees::NominalMachineUseSite::Expression(expression)),
                ..
            } => Some(*expression),
            _ => None,
        })
        .expect("literal consumer retains its exact source call");
    let ExpressionNode::Call(call) = original.typed.expression_table.expression(expression) else {
        panic!("captured literal consumer call")
    };
    let argument = original
        .typed
        .expression_table
        .expression_handles(call.arguments)[0];
    let ExpressionNode::ArrayLiteral(leaves) = original.typed.expression_table.expression(argument)
    else {
        panic!("authored array literal argument")
    };
    let leaves = original
        .typed
        .expression_table
        .expression_handles(*leaves)
        .to_vec();
    let mut changed = original.clone();
    let replacement = changed.typed.expression_table.expression(leaves[1]).clone();
    *changed.typed.expression_table.expression_mut(leaves[0]) = replacement;
    reject(
        &changed,
        "authored literal leaf changed under retained operand",
    );

    let mut changed = original;
    let elements = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source: CheckedArrayConstructionSource::CallArgument { .. },
                elements,
                ..
            } => Some(elements),
            _ => None,
        })
        .expect("literal argument constructor");
    elements.swap(0, 1);
    reject(&changed, "retained literal argument operands changed order");
}

#[test]
fn literal_array_leaf_roots_reject_reassigned_call_formal_and_element_custody() {
    let original = custody_fixture(false);
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("literal leaf roots lower before corruption");
    let pure = original
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .filter(|(_, binding)| {
            matches!(
                binding.role,
                CheckedScalarExpressionRole::ArrayElement {
                    source: CheckedArrayConstructionSource::CallArgument { .. },
                    ..
                }
            )
        })
        .map(|(handle, binding)| (handle, binding.clone()))
        .collect::<Vec<_>>();
    let computed = original
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            matches!(
                root.role,
                CheckedScalarExpressionRole::ArrayElement {
                    source: CheckedArrayConstructionSource::CallArgument { .. },
                    ..
                }
            )
        })
        .map(|(handle, root)| (handle, root.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        pure.len(),
        2,
        "each literal retains its pure parameter occurrence"
    );
    assert_eq!(
        computed.len(),
        2,
        "each literal retains its nested scalar call occurrence"
    );
    for mutation in [
        "pure owner",
        "pure element",
        "pure source expression",
        "computed owner",
        "computed element",
        "computed root",
        "computed source statement",
    ] {
        let mut changed = original.clone();
        match mutation {
            "pure owner" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(pure[0].0)
                    .role = pure[1].1.role
            }
            "pure element" => {
                let CheckedScalarExpressionRole::ArrayElement {
                    element_ordinal, ..
                } = &mut changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(pure[0].0)
                    .role
                else {
                    unreachable!()
                };
                *element_ordinal = 1;
            }
            "pure source expression" => {
                changed
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .get_mut(pure[0].0)
                    .expression = pure[1].1.expression
            }
            "computed owner" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(computed[0].0)
                    .role = computed[1].1.role
            }
            "computed element" => {
                let CheckedScalarExpressionRole::ArrayElement {
                    element_ordinal, ..
                } = &mut changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(computed[0].0)
                    .role
                else {
                    unreachable!()
                };
                *element_ordinal = 0;
            }
            "computed root" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(computed[0].0)
                    .root = computed[1].1.root
            }
            "computed source statement" => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(computed[0].0)
                    .statement_ordinal = u32::MAX
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn scalar_completion_retains_unit_value_requirements() {
    let source = "machine touch(value: u8) requires value == value {}
        machine answer(row: [u8; 2], value: u8) -> u8 { value }
        machine selected() -> u8 { touch(42u8); answer([7, 9], 42u8) }";
    assert_eq!(
        execute(source, &[]),
        TerminalExecutionResult::Scalar(byte(42))
    );
}
