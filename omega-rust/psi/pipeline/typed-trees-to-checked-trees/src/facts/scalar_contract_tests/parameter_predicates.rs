use super::*;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
};
use typed_trees::expression::ExpressionNode;
use typed_trees::types::PrimitiveType;

fn plan(program: &TypedTrees) -> checked_trees::ClosedScalarValueContractPlan {
    build_closed_scalar_value_contract_plan(
        program,
        program.machines().first().unwrap(),
        &CheckedOperatorFacts::default(),
    )
}

#[test]
fn contract_positions_keep_formals_before_the_result() {
    let program = typed(
        r#"
        machine value(flag: bool, input: u16) -> u16
        requires input < 256u16
        ensures result == input
        { input }
    "#,
    );
    let plan = plan(&program);
    let [
        Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::IntegerComparison {
            left,
            ..
        })),
    ] = plan.requires()
    else {
        panic!("numeric requirement");
    };
    assert!(matches!(
        left.as_ref(),
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U16
        }
    ));
    let [
        Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left,
            right,
        })),
    ] = plan.ensures()
    else {
        panic!("result equality");
    };
    assert!(matches!(
        left.as_ref(),
        CheckedScalarExpression::Parameter {
            position: 2,
            primitive_type: PrimitiveType::U16
        }
    ));
    assert!(matches!(
        right.as_ref(),
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U16
        }
    ));
}

#[test]
fn authored_result_parameter_shadows_the_reserved_result() {
    let program = typed(
        "machine value(result: u16) -> u16\nrequires result < 256u16\nensures result == 7u16\n{ 999u16 }",
    );
    let plan = plan(&program);
    let [
        Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::IntegerComparison {
            left,
            ..
        })),
    ] = plan.ensures()
    else {
        panic!("formal equality");
    };
    assert!(matches!(
        left.as_ref(),
        CheckedScalarExpression::Parameter { position: 0, .. }
    ));
}

#[test]
fn mutable_formal_ensures_cannot_claim_an_entry_snapshot() {
    let program = typed(
        "machine value(mut input: u16) -> u16\nrequires input < 256u16\nensures result == input\n{ input }",
    );
    let plan = plan(&program);
    assert!(matches!(
        plan.requires(),
        [Some(ClosedScalarContractValue::Predicate(_))]
    ));
    assert_eq!(plan.ensures(), &[None]);
}

#[test]
fn missing_formal_symbols_do_not_fall_back_to_spelling() {
    let mut program = typed(
        "machine value(input: u16) -> u16\nrequires input < 256u16\nensures result == input\n{ input }",
    );
    let names = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Name(path) if path.symbol.is_valid())
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    for handle in names {
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(handle) else {
            unreachable!();
        };
        path.symbol = symbols::SymbolHandle::invalid();
        path.head_symbol = symbols::SymbolHandle::invalid();
    }
    let plan = plan(&program);
    assert_eq!(plan.requires(), &[None]);
    assert_eq!(plan.ensures(), &[None]);
}

#[test]
fn formal_predicates_reject_selected_operators_and_nonliteral_arithmetic() {
    for source in [
        "boundary operator < Meaning::before(left: u16, right: u16) -> bool; machine value(input: u16) -> u16\nrequires input < 256u16\nensures result < input\n{ input }",
        "machine value(input: u16) -> u16\nrequires input + 1u16 < 256u16\nensures result == input + 1u16\n{ input }",
    ] {
        let program = typed(source);
        let plan = plan(&program);
        assert_eq!(plan.requires(), &[None]);
        assert_eq!(plan.ensures(), &[None]);
    }
}

#[test]
fn literal_parameter_ranges_append_native_requirements() {
    for range in ["[0..=255]", "[0..256]", "[0..=200, 0..=255]"] {
        let program = typed(&format!(
            r#"
            boundary operator <= Meaning::before(left: u16, right: u16) -> bool;
            machine value(input: u16 {range}) -> u16
            requires 7u16 == 7u16
            ensures result == input
            {{ input }}
        "#
        ));
        let plan = plan(&program);
        assert!(matches!(
            plan.requires()[0],
            Some(ClosedScalarContractValue::Integer(_))
        ));
        assert_eq!(
            plan.requires().len(),
            if range.contains(',') { 3 } else { 2 }
        );
        assert!(plan.requires()[1..].iter().all(|requirement| matches!(
            requirement,
            Some(ClosedScalarContractValue::Predicate(
                CheckedBooleanExpression::And { .. }
            ))
        )));
    }
}

#[test]
fn unsupported_present_ranges_are_not_silently_erased() {
    for parameter in [
        "input: u16 [0..=limit]",
        "input: u16 [0..=128 + 127]",
        "input: u16 [0..=255] in Wrapping",
    ] {
        let program = typed(&format!(
            "machine value(limit: u16, {parameter}) -> u16\nrequires 7u16 == 7u16\nensures result == input\n{{ input }}"
        ));
        let plan = plan(&program);
        assert_eq!(plan.requires().len(), 2, "{parameter}");
        assert!(plan.requires()[1].is_none(), "{parameter}");
    }
}
