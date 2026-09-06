use super::*;
use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};

#[test]
fn a_body_write_cannot_make_a_false_entry_crash_route_true() {
    for body in [
        "flag = true; crash Trap;",
        "flag = true; transition flag { true -> failed() false -> false } state failed() -> bool { crash Trap; }",
    ] {
        let source = format!(
            r#"
        machine value(mut flag: bool) -> bool
        requires !flag
        crashes Trap flag
        {{ {body} }}
    "#
        );
        let diagnostics = match lower_typed_trees(parse_typed_trees(&source)) {
            Err(diagnostics) => diagnostics,
            Ok(_) => {
                panic!("the published entry route stays false after the body changes flag: {body}")
            }
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uncovered Trap crash")),
            "{diagnostics:#?}"
        );
    }
}

fn assert_entry_parameters(expression: &CheckedBooleanExpression, expected: &[usize]) {
    fn scalar(expression: &CheckedScalarExpression, positions: &mut Vec<usize>) {
        match expression {
            CheckedScalarExpression::Parameter { position, .. } => positions.push(*position),
            CheckedScalarExpression::StorageRead { .. } | CheckedScalarExpression::Local { .. } => {
                panic!("entry contract contains body storage: {expression:?}")
            }
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                scalar(left, positions);
                scalar(right, positions);
            }
            CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
            | CheckedScalarExpression::IntegerWiden { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. } => {
                scalar(operand, positions)
            }
            CheckedScalarExpression::Boolean(expression) => boolean(expression, positions),
            _ => {}
        }
    }
    fn boolean(expression: &CheckedBooleanExpression, positions: &mut Vec<usize>) {
        match expression {
            CheckedBooleanExpression::Parameter { position } => positions.push(*position),
            CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::Local { .. } => {
                panic!("entry contract contains body storage: {expression:?}")
            }
            CheckedBooleanExpression::Not(operand) => boolean(operand, positions),
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right }
            | CheckedBooleanExpression::Equal { left, right } => {
                boolean(left, positions);
                boolean(right, positions);
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                scalar(left, positions);
                scalar(right, positions);
            }
            _ => {}
        }
    }
    let mut positions = Vec::new();
    boolean(expression, &mut positions);
    assert_eq!(positions, expected, "{expression:?}");
}

#[test]
fn mutable_boolean_requires_retains_entry_operand_and_body_retains_current_storage() {
    let checked = lower_typed_trees(parse_typed_trees(
        "machine value(mut input: bool) -> bool requires input { input = false; input }",
    ))
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .unwrap();
    let requirements = contract
        .crash
        .structural_runtime_requirements()
        .expect("complete entry requirement");
    assert_eq!(requirements.len(), 1);
    assert_entry_parameters(&requirements[0], &[0]);
    let state = &checked.machine_states(machine)[0];
    let returned = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            state.symbol,
            1,
            checked_trees::CheckedScalarExpressionRole::Return,
        )
        .expect("current Boolean return");
    assert!(
        matches!(returned, CheckedScalarExpression::Boolean(expression)
        if matches!(expression.as_ref(), CheckedBooleanExpression::StorageRead { symbol }
            if *symbol == checked.state_parameters(state)[0].symbol))
    );
}

#[test]
fn published_mutable_parameter_crash_predicates_are_entry_snapshots() {
    for (parameters, predicate, expected) in [
        ("mut input: bool", "input", vec![0]),
        (
            "first: bool, mut input: bool",
            "first && !input",
            vec![0, 1],
        ),
        ("mut input: u32", "input == 7u32", vec![0]),
    ] {
        let source = format!("machine value({parameters}) crashes Trap {predicate} {{}}");
        let checked = lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let contract = checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .unwrap();
        let [bucket] = contract.crash.published() else {
            panic!("one crash bucket")
        };
        let [checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
        else {
            panic!("one route")
        };
        assert_entry_parameters(
            predicate
                .scalar_expression()
                .expect("checked entry predicate"),
            &expected,
        );
    }
}

#[test]
fn current_mutable_guard_cannot_cover_a_call_with_a_false_entry_crash_route() {
    let source = r#"
        machine trigger() -> bool crashes Trap { crash Trap; }
        machine value(mut input: bool) -> bool
        requires !input
        crashes Trap input
        {
            input = true;
            transition input { true -> invoke() false -> false }
            state invoke() -> bool { trigger() }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("current mutable state is not the published entry route");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered Trap crash route")),
        "{diagnostics:#?}"
    );
}
