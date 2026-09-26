use super::{checked_values_for, operator_with_placeholder_operands};
use crate::operators::build_operator_facts;
use crate::tests::SymbolHandle;
use crate::tests::front_end::checked_program;
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::{ExpressionNode, TableIndexedExpression};

#[test]
fn records_checked_float_policy_adapters_from_operand_domains() {
    let source = r#"
        boundary operator + Float::add(left: f32, right: f32) -> f32;
        boundary operator + Float::add(left: f64, right: f64) -> f64;
        boundary operator - Float::subtract(left: f64, right: f64) -> f64;
        boundary operator * Float::multiply(left: f32, right: f32) -> f32;

        data Main {
            saturated_field: f32 in Saturating;
            trapped_field: f64 in Trapping;
        }

        machine Main::combine(
            &mut self,
            saturated: f32 in Saturating,
            trapped: f64 in Trapping,
            quiet: f32
        ) {
            let saturated_result: f32 = (saturated + 1.0f32) as f32;
            let trapped_result: f64 = (trapped + 1.0) as f64;
            let reversed_trapped: f64 = 0.0 - trapped;
            let nested_saturated: f32 = ((saturated * saturated) + 1.0f32) as f32;
            let quiet_result: f32 = quiet + 1.0f32;
            self.trapped_field = (0.0 - self.trapped_field) as f64 in Trapping;
            self.saturated_field =
                ((self.saturated_field * self.saturated_field) + 1.0f32) as f32 in Saturating;
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    let adapters = checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.policy_adapter)
        .collect::<Vec<_>>();

    assert!(adapters.contains(
        &checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
            format: numerics::float_semantics::FloatFormat::BINARY32,
        }
    ));
    assert!(adapters.contains(
        &checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
            format: numerics::float_semantics::FloatFormat::BINARY64,
        }
    ));
    assert!(adapters.contains(&checked_trees::CheckedArithmeticPolicyAdapter::None));
    assert_eq!(
        adapters
            .iter()
            .filter(|adapter| matches!(
                adapter,
                checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                    format: numerics::float_semantics::FloatFormat::BINARY32,
                }
            ))
            .count(),
        5,
        "parameter and attached-data nested arithmetic nodes retain saturation",
    );
    assert_eq!(
        adapters
            .iter()
            .filter(|adapter| matches!(
                adapter,
                checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                    format: numerics::float_semantics::FloatFormat::BINARY64,
                }
            ))
            .count(),
        3,
        "a contextual literal on the left retains parameter or attached-data policy",
    );
}

#[test]
fn records_checked_named_float_policy_adapters() {
    let source = r#"
        data F32 {}
        data F64 {}

        boundary operator F32::multiply_then_add(
            left: f32 in Saturating,
            right: f32 in Saturating,
            addend: f32 in Saturating
        ) -> f32;
        boundary operator F64::negate(value: f64 in Trapping) -> f64;
        boundary operator F32::is_finite(value: f32) -> bool;

        data Main {}

        machine Main::combine(
            &self,
            saturated: f32 in Saturating,
            trapped: f64 in Trapping,
            quiet: f32
        ) {
            let saturated_result: f32 =
                F32::multiply_then_add(saturated, saturated, saturated) as f32;
            let trapped_result: f64 = F64::negate(trapped) as f64;
            let classification: bool = F32::is_finite(quiet);
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    let adapters = checked
        .facts
        .operators
        .named_uses()
        .map(|operator_use| operator_use.policy_adapter)
        .collect::<Vec<_>>();

    assert!(adapters.contains(
        &checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
            format: numerics::float_semantics::FloatFormat::BINARY32,
        }
    ));
    assert!(adapters.contains(
        &checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
            format: numerics::float_semantics::FloatFormat::BINARY64,
        }
    ));
    assert!(adapters.contains(&checked_trees::CheckedArithmeticPolicyAdapter::None));
}

#[test]
fn resolves_spelled_operator_from_named_call_result_type() {
    let source = r#"
        data F32 {}

        boundary operator + Float::add(left: f32, right: f32) -> f32;
        boundary operator F32::multiply_then_add(
            left: f32,
            right: f32,
            addend: f32
        ) -> f32;

        data Main {}

        machine Main::combine(&self, left: f32, right: f32, addend: f32) {
            let result: f32 =
                F32::multiply_then_add(left, right, addend) + 0.0f32;
        }

        machine Main::main(&mut self) {}
    "#;

    let checked = checked_program(source);
    let outer_add = checked
        .facts
        .operators
        .resolved_uses()
        .find(|operator_use| {
            operator_use.spelling == OperatorSpelling::Add
                && matches!(
                    checked
                        .typed
                        .expression_table
                        .expression(operator_use.expression),
                    ExpressionNode::Binary(binary)
                        if matches!(
                            checked.typed.expression_table.expression(binary.left),
                            ExpressionNode::Call(_)
                        )
                )
        })
        .expect("named call result must type the surrounding spelled operator");

    let selected = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == outer_add.selected_operator_symbol)
        .expect("outer add must retain its selected operator");
    let path = checked.typed.operator_path_members(selected.name);
    assert_eq!(
        path.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
        ["Float", "add"]
    );
}

#[test]
fn records_ambiguous_operator_spelling_status() {
    let first_candidate = SymbolHandle::from_arena_index(90);
    let second_candidate = SymbolHandle::from_arena_index(91);

    let mut program = typed_trees::TypedTrees::default();
    let first_operator =
        operator_with_placeholder_operands(&mut program, first_candidate, OperatorSpelling::Index);
    program.push_operator(first_operator);
    let second_operator =
        operator_with_placeholder_operands(&mut program, second_candidate, OperatorSpelling::Index);
    program.push_operator(second_operator);

    let collection = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index,
            }));

    let values = checked_values_for([indexed]);
    let facts = build_operator_facts(&program, &values);
    let indexed_use = facts.expression_use(indexed).expect("indexed use");

    assert_eq!(indexed_use.spelling, OperatorSpelling::Index);
    assert_eq!(
        indexed_use.status,
        checked_trees::CheckedOperatorResolutionStatus::Ambiguous
    );
    assert_eq!(indexed_use.candidate_count, 2);
    assert_eq!(
        facts.candidate_symbols(indexed_use).collect::<Vec<_>>(),
        vec![first_candidate, second_candidate]
    );
    assert!(!indexed_use.selected_operator_symbol.is_valid());
}
