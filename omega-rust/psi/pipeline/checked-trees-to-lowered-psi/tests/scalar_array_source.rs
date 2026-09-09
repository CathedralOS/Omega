//! Array results rejoin exact authored bindings and initializer contents.

use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

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
    use typed_trees::types::PrimitiveType;

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
        let other_literal = other_elements[0].clone();
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
                    elements[0] = other_literal;
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
