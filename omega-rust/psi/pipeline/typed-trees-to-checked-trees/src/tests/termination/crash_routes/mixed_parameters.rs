use super::*;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use typed_trees::types::PrimitiveType;

fn checked_fixture(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn predicate(checked: &checked_trees::CheckedTrees) -> &checked_trees::CrashPredicateIdentity {
    let contract = checked
        .facts
        .contract_plans
        .for_machine(symbol_of_checked(checked, "inspect"))
        .expect("contract");
    let [bucket] = contract.crash.published() else {
        panic!("one crash bucket")
    };
    let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards() else {
        panic!("one guarded crash route")
    };
    predicate
}

fn conjunction_leaves<'a>(
    expression: &'a CheckedBooleanExpression,
    leaves: &mut Vec<&'a CheckedBooleanExpression>,
) {
    if let CheckedBooleanExpression::And { left, right } = expression {
        conjunction_leaves(left, leaves);
        conjunction_leaves(right, leaves);
    } else {
        leaves.push(expression);
    }
}

#[test]
fn mixed_integer_crash_parameters_keep_dense_scalars_and_authored_field_roots() {
    for (signature, right_receiver, flag_position, second_position, left_root, right_root) in [
        (
            "first: i32, storage: &Values, enabled: bool, second: i32",
            "storage",
            1,
            2,
            1,
            1,
        ),
        (
            "storage: &Values, first: i32, other: &Values, second: i32, enabled: bool",
            "other",
            2,
            1,
            0,
            2,
        ),
    ] {
        let checked = checked_fixture(&format!(
            r#"
            data Values [copy] {{ left: i32; right: i32; }}
            machine inspect({signature})
            crashes Abort
                enabled && first == storage.left && second == {right_receiver}.right
            {{}}
        "#
        ));
        let expression = predicate(&checked)
            .scalar_expression()
            .expect("mixed structured predicate");
        let mut leaves = Vec::new();
        conjunction_leaves(expression, &mut leaves);
        assert_eq!(leaves.len(), 3);
        assert_eq!(
            *leaves[0],
            CheckedBooleanExpression::Parameter {
                position: flag_position
            }
        );
        for (leaf, scalar_position, field_position, field) in [
            (leaves[1], 0, left_root, "left"),
            (leaves[2], second_position, right_root, "right"),
        ] {
            assert_eq!(
                *leaf,
                CheckedBooleanExpression::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::Equal,
                    left: Box::new(CheckedScalarExpression::Parameter {
                        position: scalar_position,
                        primitive_type: PrimitiveType::I32,
                    }),
                    right: Box::new(CheckedScalarExpression::StructuralParameterField {
                        parameter_position: field_position,
                        path: vec![CheckedStructuralPredicatePathSegment::Field(
                            field.to_owned()
                        )],
                        primitive_type: PrimitiveType::I32,
                    }),
                }
            );
        }
    }
}

#[test]
fn mixed_integer_parameter_arithmetic_retains_its_selected_policy() {
    for (policy, expected_kind) in [
        ("Wrapping", CheckedIntegerBinaryKind::WrappingAdd),
        ("Saturating", CheckedIntegerBinaryKind::SaturatingAdd),
    ] {
        let checked = checked_fixture(&format!(
            r#"
            data Values [copy] {{ value: i32 in {policy}; }}
            machine inspect(storage: &Values, value: i32 in {policy})
            crashes Abort
                value + storage.value == value
            {{}}
        "#
        ));
        let CheckedBooleanExpression::IntegerComparison { left, right, .. } = predicate(&checked)
            .scalar_expression()
            .expect("policy-preserving mixed predicate")
        else {
            panic!("integer comparison")
        };
        assert!(
            matches!(left.as_ref(), CheckedScalarExpression::IntegerBinary { kind, left, right, .. }
            if *kind == expected_kind
                && matches!(left.as_ref(), CheckedScalarExpression::Parameter { position: 0, primitive_type: PrimitiveType::I32 })
                && matches!(right.as_ref(), CheckedScalarExpression::StructuralParameterField { parameter_position: 0, .. }))
        );
        assert_eq!(
            right.as_ref(),
            &CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::I32
            }
        );
    }
}

#[test]
fn mixed_address_parameters_do_not_gain_fixed_integer_crash_meaning() {
    let checked = checked_fixture(
        r#"
        data Values [copy] { value: i32; }
        machine inspect(storage: &Values, left: addr, right: addr)
        crashes Abort
            left == right
        {}
    "#,
    );
    assert!(predicate(&checked).scalar_expression().is_none());
}
