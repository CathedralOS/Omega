//! Array results rejoin exact authored bindings and initializer contents.

#[path = "scalar_array_source/call_arguments.rs"]
mod call_arguments;
#[path = "scalar_array_source/call_results.rs"]
mod call_results;
#[path = "scalar_array_source/computation_arguments.rs"]
mod computation_arguments;
#[path = "scalar_array_source/cyclic.rs"]
mod cyclic;
#[path = "scalar_array_source/floating.rs"]
mod floating;
#[path = "scalar_array_source/literal_arguments.rs"]
mod literal_arguments;

use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitEffectOperationPlan,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
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
    crate::front_end::checked_program(&source)
}

fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            checked,
            TerminalMachineSelection::Name("selected")
        )
        .is_err(),
        "source custody must reject {mutation}"
    );
}

#[test]
fn empty_array_catalog_cannot_change_primitive_or_nested_dimensions_under_the_same_identity() {
    use checked_trees::CheckedUnitStructuralTypeShape;

    for (carrier, primitive_type) in [
        ("[u8; 0]", PrimitiveType::U8),
        ("[[u8; 2]; 0]", PrimitiveType::U8),
        ("[f32; 0]", PrimitiveType::F32),
        ("[[f64; 2]; 0]", PrimitiveType::F64),
    ] {
        let original =
            crate::front_end::checked_program(&format!("machine selected() -> {carrier} {{ [] }}"));
        checked_trees_to_lowered_psi::lower_machine(
            &original,
            TerminalMachineSelection::Name("selected"),
        )
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
                    CheckedUnitStructuralTypeShape::PrimitiveScalar(actual) if actual == primitive_type
                )
            })
            .expect("empty array retains its primitive declaration");
        primitive.shape = CheckedUnitStructuralTypeShape::PrimitiveScalar(PrimitiveType::Bool);
        reject(
            &changed,
            "empty array primitive changed under retained identity",
        );

        if carrier.starts_with("[[") {
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
            ..
        } = &plan.operations[*second]
        else {
            panic!("second array");
        };
        let other_result = other_result.clone();
        let other_operand = other_elements[0].clone();
        match mutation {
            "deleted result" => plan.structural_result = None,
            "another returned binding" => plan.structural_result = Some(other_result.into()),
            "unknown returned ordinal" => {
                plan.structural_result.as_mut().unwrap().source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: u32::MAX,
                    }
            }
            "constructor ordinal" => {
                let CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. } =
                    &mut plan.operations[*first]
                else {
                    panic!("first array");
                };
                result.binding_ordinal = other_result.binding_ordinal;
                plan.structural_result.as_mut().unwrap().source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: other_result.binding_ordinal,
                    };
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

fn computed_fixture(local: bool) -> CheckedTrees {
    let returned = if local {
        "let row: [u8; 4] = [first, saved, identity(second), identity(first)]; row"
    } else {
        "[first, saved, identity(second), identity(first)]"
    };
    crate::front_end::checked_program(&format!(
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

    let original = crate::front_end::checked_program(
        "machine identity(value: u8) -> u8 { value }
         machine selected(first: u8, second: u8) -> [u8; 2] {
             [first & second, identity(first) | second]
         }",
    );
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
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
                        == (CheckedScalarExpressionRole::ArrayElement {
                            source: checked_trees::CheckedArrayConstructionSource::Statement,
                            element_ordinal: 0,
                        })
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

    let original =
        crate::front_end::checked_program("machine selected() -> [bool; 2] { [true, false] }");
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
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
        if retained.role
            == (CheckedScalarExpressionRole::ArrayElement {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                element_ordinal: 0,
            })
        {
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
    let original = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
    .expect("nested exact literal casts lower");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
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
fn array_operator_selection_cannot_change_while_value_and_source_stay_fixed() {
    let mut original = crate::front_end::checked_program(
        "machine selected(input: u8) -> [u16; 1] { [(input as u16) + 1u16] }",
    );
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
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
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
