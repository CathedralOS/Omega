//! Array results rejoin exact authored bindings and initializer contents.

use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitEffectOperationPlan,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

fn fixture(returned: &str) -> CheckedTrees {
    let source = format!(
        "data Sizes {{}}
         const Sizes::ROWS: [[u8; 2]; 2] = [[7, 9], [11, 13]];
         machine selected() -> [u8; 2] {{
             let first: [u8; 2] = Sizes::ROWS[0];
             let second: [u8; 2] = Sizes::ROWS[1];
             {returned}
         }}"
    );
    checked_source(&source)
}

fn checked_source(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check selected array locals")
}

fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        checked_trees_to_lowered_psi::lower_machine(checked, "selected").is_err(),
        "source custody must reject {mutation}"
    );
}

#[test]
fn same_typed_array_locals_return_the_authored_binding_contents() {
    for (returned, expected) in [("first", [7, 9]), ("second", [11, 13])] {
        let checked = fixture(returned);
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
            .expect("lower ordinary array locals and selected return");
        let semantic =
            terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
        let proof =
            terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
        let TerminalExecutionResult::ScalarArray(result) =
            interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
                .expect("execute decoded array return")
        else {
            panic!("selected binding must return its array contents");
        };
        let result_type = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .and_then(|machine| machine.result.structural())
            .expect("array result signature")
            .structural_type;
        assert_eq!(result.value.structural_type, result_type);
        assert_eq!(
            result.value.elements,
            expected.map(|value| TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(value),
            })
        );
    }
}

#[test]
fn empty_array_catalog_cannot_change_primitive_or_nested_dimensions_under_the_same_identity() {
    use checked_trees::CheckedUnitStructuralTypeShape;

    for carrier in ["[u8; 0]", "[[u8; 2]; 0]"] {
        let original = checked_source(&format!("machine selected() -> {carrier} {{ [] }}"));
        checked_trees_to_lowered_psi::lower_machine(&original, "selected")
            .expect("empty array source has complete declared shape");

        let mut changed = original.clone();
        let primitive = changed
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter_mut()
            .find(|declaration| {
                matches!(
                    declaration.shape,
                    CheckedUnitStructuralTypeShape::PrimitiveScalar(PrimitiveType::U8)
                )
            })
            .expect("empty array retains its primitive declaration");
        primitive.shape = CheckedUnitStructuralTypeShape::PrimitiveScalar(PrimitiveType::Bool);
        reject(
            &changed,
            "empty array primitive changed under retained identity",
        );

        if carrier == "[[u8; 2]; 0]" {
            let mut changed = original;
            let nested = changed
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter_mut()
                .find(|declaration| {
                    matches!(
                        declaration.shape,
                        CheckedUnitStructuralTypeShape::FixedArray { length: 2, .. }
                    )
                })
                .expect("empty outer array retains its nested dimension");
            let CheckedUnitStructuralTypeShape::FixedArray { length, .. } = &mut nested.shape
            else {
                panic!("nested array declaration");
            };
            *length = 3;
            reject(
                &changed,
                "nested dimension changed below an empty outer array",
            );
        }
    }
}

#[test]
fn changed_array_result_and_constructor_facts_reject() {
    let original = fixture("first");
    for mutation in [
        "deleted result",
        "another returned binding",
        "unknown returned ordinal",
        "constructor ordinal",
        "dropped returned constructor",
        "dropped unreturned constructor",
        "reordered constructors",
        "reordered constructor leaves",
        "changed constructor literal",
    ] {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|machine| machine.structural_result.is_some())
            .expect("array result plan");
        let constructors = plan
            .operations
            .iter()
            .enumerate()
            .filter_map(|(position, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                )
                .then_some(position)
            })
            .collect::<Vec<_>>();
        let [first, second] = constructors.as_slice() else {
            panic!("two authored array constructors");
        };
        let CheckedUnitEffectOperationPlan::EstablishScalarArray {
            result: other_result,
            elements: other_elements,
        } = &plan.operations[*second]
        else {
            panic!("second array");
        };
        let other_result = other_result.clone();
        let other_operand = other_elements[0].clone();
        match mutation {
            "deleted result" => plan.structural_result = None,
            "another returned binding" => plan.structural_result = Some(other_result),
            "unknown returned ordinal" => {
                plan.structural_result.as_mut().unwrap().binding_ordinal = u32::MAX
            }
            "constructor ordinal" => {
                let CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. } =
                    &mut plan.operations[*first]
                else {
                    panic!("first array");
                };
                result.binding_ordinal = other_result.binding_ordinal;
                plan.structural_result.as_mut().unwrap().binding_ordinal =
                    other_result.binding_ordinal;
            }
            "dropped returned constructor" => {
                plan.operations.remove(*first);
            }
            "dropped unreturned constructor" => {
                plan.operations.remove(*second);
            }
            "reordered constructors" => plan.operations.swap(*first, *second),
            "reordered constructor leaves" | "changed constructor literal" => {
                let CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. } =
                    &mut plan.operations[*first]
                else {
                    panic!("first array");
                };
                if mutation == "reordered constructor leaves" {
                    elements.swap(0, 1);
                } else {
                    elements[0] = other_operand;
                }
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

fn selected_source(
    checked: &CheckedTrees,
) -> (arena::HandleSpan<StatementNode>, Vec<ExpressionHandle>) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "selected")
        .expect("selected machine");
    let state = &checked.typed.machine_states(machine)[0];
    let expressions = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .map(|statement| match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Expression(expression) => *expression,
            _ => panic!("array fixture statement"),
        })
        .collect();
    (state.statement_nodes, expressions)
}

fn row_elements(
    checked: &CheckedTrees,
    projection: ExpressionHandle,
    row: usize,
) -> Vec<ExpressionHandle> {
    let ExpressionNode::Indexed(indexed) = checked.typed.expression_table.expression(projection)
    else {
        panic!("authored array projection");
    };
    let ExpressionNode::ArrayLiteral(rows) = checked
        .typed
        .expression_table
        .expression(indexed.collection)
    else {
        panic!("constant array initializer");
    };
    let selected = checked.typed.expression_table.expression_handles(*rows)[row];
    let ExpressionNode::ArrayLiteral(elements) =
        checked.typed.expression_table.expression(selected)
    else {
        panic!("array row");
    };
    checked
        .typed
        .expression_table
        .expression_handles(*elements)
        .to_vec()
}

#[test]
fn changed_authored_array_payload_order_binding_or_sibling_rejects() {
    let original = fixture("first");
    for mutation in [
        "changed typed literal",
        "reordered typed leaves",
        "changed return name",
        "reordered source constructors",
        "invalid unselected sibling",
    ] {
        let mut changed = original.clone();
        let (statements, expressions) = selected_source(&changed);
        let leaves = row_elements(&changed, expressions[0], 0);
        let siblings = row_elements(&changed, expressions[0], 1);
        match mutation {
            "changed typed literal" => {
                let replacement = changed
                    .typed
                    .expression_table
                    .expression(siblings[0])
                    .clone();
                *changed.typed.expression_table.expression_mut(leaves[0]) = replacement;
            }
            "reordered typed leaves" => {
                let first = changed.typed.expression_table.expression(leaves[0]).clone();
                let second = changed.typed.expression_table.expression(leaves[1]).clone();
                *changed.typed.expression_table.expression_mut(leaves[0]) = second;
                *changed.typed.expression_table.expression_mut(leaves[1]) = first;
            }
            "changed return name" => {
                let StatementNode::LocalData(second) =
                    &changed.typed.statement_table.statements(statements)[1]
                else {
                    panic!("second binding");
                };
                let symbol = second.symbol;
                let ExpressionNode::Name(path) = changed
                    .typed
                    .expression_table
                    .expression_mut(expressions[2])
                else {
                    panic!("returned name");
                };
                path.symbol = symbol;
            }
            "reordered source constructors" => changed
                .typed
                .statement_table
                .statements_mut(statements)
                .swap(0, 1),
            "invalid unselected sibling" => {
                *changed.typed.expression_table.expression_mut(siblings[0]) =
                    ExpressionNode::Name(Default::default());
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

fn computed_fixture(local: bool) -> CheckedTrees {
    let returned = if local {
        "let row: [u8; 4] = [first, saved, identity(second), identity(first)]; row"
    } else {
        "[first, saved, identity(second), identity(first)]"
    };
    checked_source(&format!(
        "machine identity(value: u8) -> u8 {{ value }}
         machine selected(first: u8, second: u8) -> [u8; 4] {{
             let saved: u8 = second;
             {returned}
         }}"
    ))
}

fn constructor_elements(checked: &mut CheckedTrees) -> &mut Vec<CheckedCallScalarArgument> {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.structural_result.is_some())
        .expect("selected array machine")
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. } => Some(elements),
            _ => None,
        })
        .expect("computed array constructor")
}

#[test]
fn array_parameters_locals_and_calls_execute_in_authored_leaf_order() {
    for local in [false, true] {
        let mut checked = computed_fixture(local);
        assert!(matches!(
            constructor_elements(&mut checked).as_slice(),
            [
                CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter { .. }),
                CheckedCallScalarArgument::Pure(CheckedScalarExpression::Local { .. }),
                CheckedCallScalarArgument::Computation(_),
                CheckedCallScalarArgument::Computation(_),
            ]
        ));
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
            .expect("lower evaluated array operands");
        let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
        let byte = |value| TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(value),
        };
        let TerminalExecutionResult::ScalarArray(result) = interpret_terminal_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[byte(7), byte(9)],
        )
        .expect("execute decoded computed array") else {
            panic!("computed array result");
        };
        assert_eq!(result.value.elements, [byte(7), byte(9), byte(9), byte(7)]);
    }
}

#[test]
fn substituted_array_pure_operands_and_computation_roots_reject() {
    for local in [false, true] {
        let original = computed_fixture(local);
        checked_trees_to_lowered_psi::lower_machine(&original, "selected")
            .expect("computed fixture lowers before corruption");
        for mutation in [
            "pure plan",
            "parameter position",
            "primitive type",
            "computation order",
            "pure replaces call",
            "missing computation",
        ] {
            let mut changed = original.clone();
            let elements = constructor_elements(&mut changed);
            match mutation {
                "pure plan" => elements[0] = elements[1].clone(),
                "parameter position" | "primitive type" => {
                    let CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter {
                        position,
                        primitive_type,
                    }) = &mut elements[0]
                    else {
                        panic!("first element parameter");
                    };
                    if mutation == "parameter position" {
                        *position = 1;
                    } else {
                        *primitive_type = PrimitiveType::U16;
                    }
                }
                "computation order" => elements.swap(2, 3),
                "pure replaces call" => elements[2] = elements[0].clone(),
                "missing computation" => {
                    elements[2] = CheckedCallScalarArgument::Computation(arena::Handle::invalid())
                }
                _ => unreachable!(),
            }
            reject(&changed, mutation);
        }
    }
}

#[test]
fn array_element_pure_source_coordinates_and_destination_reject_corruption() {
    for local in [false, true] {
        let original = computed_fixture(local);
        let rows = original
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .filter(|(_, binding)| {
                matches!(
                    binding.role,
                    CheckedScalarExpressionRole::ArrayElement { .. }
                )
            })
            .map(|(handle, binding)| (handle, binding.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            2,
            "pure parameter and saved-local elements retain custody"
        );
        for (handle, row) in rows {
            assert_eq!(
                row.destination.is_valid(),
                local,
                "local construction and final return have distinct destinations"
            );
            for mutation in [
                "source expression",
                "element ordinal",
                "statement ordinal",
                "destination",
                "namespace order",
                "duplicate source row",
            ] {
                let mut changed = original.clone();
                let plans = &mut changed.facts.values.scalar_expressions;
                match mutation {
                    "source expression" => {
                        plans.source_bindings.get_mut(handle).expression =
                            ExpressionHandle::invalid()
                    }
                    "element ordinal" => {
                        plans.source_bindings.get_mut(handle).role =
                            CheckedScalarExpressionRole::ArrayElement {
                                element_ordinal: u32::MAX,
                            }
                    }
                    "statement ordinal" => {
                        plans.source_bindings.get_mut(handle).statement_ordinal += 1
                    }
                    "destination" => {
                        plans.source_bindings.get_mut(handle).destination = if local {
                            symbols::SymbolHandle::invalid()
                        } else {
                            row.state
                        }
                    }
                    "namespace order" => {
                        let mut names = plans.binding_symbols.span_or_empty(row.symbols).to_vec();
                        assert!(names.len() >= 2);
                        names.swap(0, 1);
                        plans.source_bindings.get_mut(handle).symbols =
                            plans.binding_symbols.insert_many(names);
                    }
                    "duplicate source row" => {
                        plans.source_bindings.append(row.clone());
                    }
                    _ => unreachable!(),
                }
                reject(&changed, mutation);
            }
        }
    }
}

#[test]
fn array_computation_source_roots_and_types_reject_corruption() {
    let original = computed_fixture(false);
    let roots = original
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| matches!(root.role, CheckedScalarExpressionRole::ArrayElement { .. }))
        .map(|(handle, root)| (handle, root.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        roots.len(),
        2,
        "each array call has its exact computation root"
    );
    let (handle, first) = &roots[0];
    let second = &roots[1].1;
    for mutation in [
        "root handle",
        "root ordinal",
        "root statement",
        "authored call",
        "root primitive",
    ] {
        let mut changed = original.clone();
        let plans = &mut changed.facts.values.scalar_computations;
        match mutation {
            "root handle" => plans.roots.get_mut(*handle).root = second.root,
            "root ordinal" => plans.roots.get_mut(*handle).role = second.role,
            "root statement" => plans.roots.get_mut(*handle).statement_ordinal += 1,
            "authored call" => {
                let replacement = plans.nodes.get(second.root).authored_root;
                plans.nodes.get_mut(first.root).authored_root = replacement;
            }
            "root primitive" => plans.nodes.get_mut(first.root).primitive_type = PrimitiveType::U16,
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn changed_authored_array_parameter_local_and_call_order_rejects() {
    let original = computed_fixture(false);
    let (_, expressions) = selected_source(&original);
    let ExpressionNode::ArrayLiteral(elements) =
        original.typed.expression_table.expression(expressions[1])
    else {
        panic!("computed array literal");
    };
    let leaves = original
        .typed
        .expression_table
        .expression_handles(*elements)
        .to_vec();
    for mutation in ["parameter becomes local", "saved initializer", "call order"] {
        let mut changed = original.clone();
        match mutation {
            "parameter becomes local" | "saved initializer" => {
                let (target, replacement) = if mutation == "parameter becomes local" {
                    (leaves[0], leaves[1])
                } else {
                    (expressions[0], leaves[0])
                };
                let replacement = changed
                    .typed
                    .expression_table
                    .expression(replacement)
                    .clone();
                *changed.typed.expression_table.expression_mut(target) = replacement;
            }
            "call order" => {
                let first = changed.typed.expression_table.expression(leaves[2]).clone();
                let second = changed.typed.expression_table.expression(leaves[3]).clone();
                *changed.typed.expression_table.expression_mut(leaves[2]) = second;
                *changed.typed.expression_table.expression_mut(leaves[3]) = first;
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn array_pure_and_computed_operators_reject_semantic_substitution() {
    use checked_trees::{CheckedIntegerBinaryKind, CheckedScalarComputationKind};
    use typed_trees::expression::BinaryOperator;

    let original = checked_source(
        "machine identity(value: u8) -> u8 { value }
         machine selected(first: u8, second: u8) -> [u8; 2] {
             [first & second, identity(first) | second]
         }",
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("pure and computed bitwise operands lower before corruption");
    let (_, expressions) = selected_source(&original);
    let ExpressionNode::ArrayLiteral(elements) =
        original.typed.expression_table.expression(expressions[0])
    else {
        panic!("operator array");
    };
    let sources = original
        .typed
        .expression_table
        .expression_handles(*elements)
        .to_vec();
    for mutation in [
        "pure source operator",
        "computed source operator",
        "pure retained operator",
        "computed retained operator",
    ] {
        let mut changed = original.clone();
        match mutation {
            "pure source operator" | "computed source operator" => {
                let index = usize::from(mutation == "computed source operator");
                let ExpressionNode::Binary(binary) = changed
                    .typed
                    .expression_table
                    .expression_mut(sources[index])
                else {
                    panic!("binary source");
                };
                binary.operator = BinaryOperator::BitwiseXor;
            }
            "pure retained operator" => {
                let CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerBinary {
                    kind,
                    ..
                }) = &mut constructor_elements(&mut changed)[0]
                else {
                    panic!("pure bitwise operand");
                };
                *kind = CheckedIntegerBinaryKind::BitwiseXor;
                for retained in &mut changed.facts.values.scalar_expressions.expressions {
                    if retained.role
                        == (CheckedScalarExpressionRole::ArrayElement { element_ordinal: 0 })
                        && let CheckedScalarExpression::IntegerBinary { kind, .. } =
                            &mut retained.expression
                    {
                        *kind = CheckedIntegerBinaryKind::BitwiseXor;
                    }
                }
            }
            "computed retained operator" => {
                let CheckedCallScalarArgument::Computation(root) =
                    constructor_elements(&mut changed)[1]
                else {
                    panic!("computed bitwise operand");
                };
                let CheckedScalarComputationKind::Apply {
                    expression: CheckedScalarExpression::IntegerBinary { kind, .. },
                    ..
                } = &mut changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(root)
                    .kind
                else {
                    panic!("computed bitwise application");
                };
                *kind = CheckedIntegerBinaryKind::BitwiseXor;
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn array_boolean_values_reject_source_and_retained_substitution() {
    use checked_trees::CheckedBooleanExpression;

    let original = checked_source("machine selected() -> [bool; 2] { [true, false] }");
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("Boolean array lowers before corruption");
    let (_, expressions) = selected_source(&original);
    let ExpressionNode::ArrayLiteral(elements) =
        original.typed.expression_table.expression(expressions[0])
    else {
        panic!("Boolean array");
    };
    let first = original
        .typed
        .expression_table
        .expression_handles(*elements)[0];
    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(first) = ExpressionNode::Boolean(false);
    reject(&changed, "changed authored Boolean literal");

    let mut changed = original.clone();
    constructor_elements(&mut changed)[0] = CheckedCallScalarArgument::Pure(
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(false))),
    );
    for retained in &mut changed.facts.values.scalar_expressions.expressions {
        if retained.role == (CheckedScalarExpressionRole::ArrayElement { element_ordinal: 0 }) {
            retained.expression = CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(false),
            ));
        }
    }
    reject(&changed, "changed retained Boolean literal");
}

#[test]
fn nested_literal_casts_preserve_the_exact_value_and_each_intermediate_fit() {
    let source = "machine byte(value: u8) -> u8 { value }
                  machine selected() -> [u64; 1] { [((300u16 as u32) as u64)] }";
    let original = checked_source(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("nested exact literal casts lower");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let TerminalExecutionResult::ScalarArray(result) =
        interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .expect("execute nested exact literal casts")
    else {
        panic!("array result");
    };
    assert_eq!(
        result.value.elements,
        [TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            value: IntegerValue::Unsigned(300),
        }]
    );
    let byte = original
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "byte")
        .unwrap();
    let narrow = original.typed.machine_states(byte)[0].return_type;
    let (_, expressions) = selected_source(&original);
    let ExpressionNode::ArrayLiteral(elements) =
        original.typed.expression_table.expression(expressions[0])
    else {
        panic!("array literal");
    };
    let root = original
        .typed
        .expression_table
        .expression_handles(*elements)[0];
    let ExpressionNode::Cast(outer) = original.typed.expression_table.expression(root) else {
        panic!("outer cast");
    };
    let inner = outer.value;
    let mut changed = original.clone();
    let ExpressionNode::Cast(cast) = changed.typed.expression_table.expression_mut(inner) else {
        panic!("inner cast");
    };
    cast.target_type = narrow;
    reject(
        &changed,
        "eliminated intermediate literal cast no longer fits",
    );
}

#[test]
fn array_boolean_expression_depth_has_no_smaller_correspondence_limit() {
    let source = format!(
        "machine selected(value: bool) -> [bool; 1] {{ [{}value] }}",
        "!".repeat(140)
    );
    let checked = checked_source(&source);
    checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
        .expect("array operands retain the ordinary scalar expression depth");
}

#[test]
fn array_operator_selection_cannot_change_while_value_and_source_stay_fixed() {
    let mut original =
        checked_source("machine selected(input: u8) -> [u16; 1] { [(input as u16) + 1u16] }");
    let (_, statements) = selected_source(&original);
    let ExpressionNode::ArrayLiteral(elements) =
        original.typed.expression_table.expression(statements[0])
    else {
        panic!("array");
    };
    let source = original
        .typed
        .expression_table
        .expression_handles(*elements)[0];
    let existing = original
        .facts
        .operators
        .uses
        .iter()
        .find_map(|(handle, selected)| (selected.expression == source).then_some(handle));
    let handle = existing.unwrap_or_else(|| {
        original
            .facts
            .operators
            .uses
            .append(checked_trees::CheckedOperatorUseFact {
                expression: source,
                spelling: language_core::OperatorSpelling::Add,
                status: checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback,
                ..Default::default()
            })
    });
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("retained builtin addition lowers");
    for mutation in ["spelling", "selected declaration", "candidate count"] {
        let mut changed = original.clone();
        let state = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|machine| machine.structural_result.is_some())
            .unwrap()
            .state;
        let selected = changed.facts.operators.uses.get_mut(handle);
        match mutation {
            "spelling" => selected.spelling = language_core::OperatorSpelling::Subtract,
            "selected declaration" => selected.selected_operator_symbol = state,
            "candidate count" => selected.candidate_count += 1,
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}
