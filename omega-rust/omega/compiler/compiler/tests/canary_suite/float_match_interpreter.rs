use super::*;

#[test]
fn selected_float_match_interpreter_preserves_ieee_values_and_effect_order() {
    let canary = pass_canary("expressions/match_float_interpretation");
    let checked = compile_to_checked(&canary.join("main.omg"), Some("macos_arm64"))
        .expect("float interpretation customer checks with exact providers");
    for format in ["32", "64"] {
        for (case, expected) in [
            ("first", 7),
            ("second", 9),
            ("fallback", 11),
            ("overlap", 7),
            ("zeros", 7),
            ("nan", 11),
            ("ordered", 7),
            ("projection", 7),
        ] {
            let machine = format!("{case}{format}");
            let outcome = checked_interpreter::interpret_entry(&checked, &machine, &[]);
            assert_eq!(outcome.error, None, "{machine}");
            assert_eq!(outcome.exit_code, expected, "{machine}");
        }
    }
}

#[test]
fn selected_float_match_interpreter_rejects_integer_runtime_substitution() {
    let canary = pass_canary("expressions/match_float_interpretation");
    let checked = compile_to_checked(&canary.join("main.omg"), Some("macos_arm64"))
        .expect("floating projection customer checks with exact providers");
    for machine in ["projection32", "projection64"] {
        let positive = checked_interpreter::interpret_entry(&checked, machine, &[]);
        assert_eq!(
            positive.error, None,
            "{machine} must work before substitution"
        );
        assert_eq!(positive.exit_code, 7, "{machine}");
    }

    let mut changed: checked_trees::CheckedTrees = (*checked).clone();
    let literals = changed
        .typed
        .expression_table
        .iter_expressions()
        .filter(|(_, expression)| {
            matches!(
                expression,
                typed_trees::expression::ExpressionNode::Float(_)
            )
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert!(
        !literals.is_empty(),
        "fixture must contain floating literals"
    );
    for literal in literals {
        *changed.typed.expression_table.expression_mut(literal) =
            typed_trees::expression::ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(1),
            );
    }
    // Indexed subjects lack scalar metadata in this interpreter. Even if both
    // runtime operands now appear integral, retained floating selection must
    // prevent them from borrowing builtin integer equality.
    for machine in ["projection32", "projection64"] {
        let outcome = checked_interpreter::interpret_entry(&changed, machine, &[]);
        assert!(
            outcome.error.is_some(),
            "{machine} cannot execute substituted operands"
        );
    }
}

#[test]
fn selected_float_match_interpreter_rejects_stale_or_substituted_execution_custody() {
    let canary = pass_canary("expressions/match_float_patterns");
    let checked = compile_to_checked(&canary.join("main.omg"), Some("macos_arm64"))
        .expect("float match customer checks");
    let positive = checked_interpreter::interpret_entry(&checked, "launch", &[]);
    assert_eq!(
        positive.error, None,
        "negative controls need working execution"
    );
    let (child_handle, child) = checked
        .facts
        .operators
        .selected_float_comparisons
        .iter()
        .next()
        .unwrap();
    let child = *child;
    let operator_use = child.operator_use();
    let site = checked
        .facts
        .operators
        .uses
        .get(operator_use)
        .application_site();
    for change in [
        "missing",
        "duplicate",
        "plan",
        "report",
        "arm",
        "requirement",
        "operands",
        "application",
        "open_application",
    ] {
        let mut changed: checked_trees::CheckedTrees = (*checked).clone();
        match change {
            "missing" => {
                changed.facts.operators.selected_float_comparisons.clear();
            }
            "duplicate" => {
                changed
                    .facts
                    .operators
                    .selected_float_comparisons
                    .append(child);
            }
            "plan" => {
                changed
                    .facts
                    .operators
                    .uses
                    .get_mut(operator_use)
                    .provider_plan_commitment =
                    checked_trees::CheckedProviderPlanCommitment::from_digest([9; 32])
            }
            "report" => {
                changed
                    .facts
                    .operators
                    .uses
                    .get_mut(operator_use)
                    .provider_plan_report_fingerprint ^= 1
            }
            "arm" => {
                let other = changed
                    .facts
                    .operators
                    .uses
                    .iter()
                    .find(|(handle, _)| *handle != operator_use)
                    .unwrap()
                    .1
                    .occurrence;
                changed
                    .facts
                    .operators
                    .uses
                    .get_mut(operator_use)
                    .occurrence = other;
            }
            "requirement" => {
                changed
                    .facts
                    .operators
                    .uses
                    .get_mut(operator_use)
                    .selected_operator_symbol = symbols::SymbolHandle::invalid()
            }
            "operands" => {
                let pattern = changed
                    .facts
                    .operators
                    .uses
                    .get(operator_use)
                    .operands(&changed.typed)
                    .unwrap()[1];
                let expression = changed.facts.operators.uses.get(operator_use).expression;
                let typed_trees::expression::ExpressionNode::Match(dispatch) =
                    changed.typed.expression_table.expression_mut(expression)
                else {
                    panic!("Match root");
                };
                dispatch.subject = pattern;
            }
            "application" => changed
                .facts
                .operators
                .boundary_applications
                .retain(|application| application.site != site),
            "open_application" => changed.facts.operators.symbolic_boundary_applications.push(
                checked_trees::CheckedSymbolicBoundaryOperatorApplicationDemand {
                    site,
                    requirement_symbol: checked
                        .facts
                        .operators
                        .uses
                        .get(operator_use)
                        .selected_operator_symbol,
                    machine_symbol: checked
                        .facts
                        .operators
                        .uses
                        .get(operator_use)
                        .origin
                        .machine_symbol()
                        .unwrap(),
                    arguments: Vec::new(),
                },
            ),
            _ => unreachable!("named test control"),
        }
        assert!(
            changed
                .facts
                .operators
                .selected_float_comparisons
                .get(child_handle)
                .validated_primitive(&changed.typed, &changed.facts.operators)
                .is_none()
                || change == "duplicate",
            "{change} must invalidate the retained child"
        );
        let outcome = checked_interpreter::interpret_entry(&changed, "launch", &[]);
        assert!(outcome.error.is_some(), "{change} cannot execute");
    }
}
