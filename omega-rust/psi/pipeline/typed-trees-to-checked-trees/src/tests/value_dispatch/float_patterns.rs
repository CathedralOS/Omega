use super::check;
use language_core::operator_spelling::OperatorSpelling;

fn source(format: &str, declarations: &str, first_pattern: &str) -> String {
    format!(
        r#"
        {declarations}
        machine identity(value: {format}) -> {format} {{ value }}
        machine choose(value: {format}, first: {format}, second: {format}) -> u64 {{
            match identity(value) {{
                {first_pattern} -> 7,
                identity(second) -> 9,
                _ -> 11
            }}
        }}
        "#
    )
}

const EQUALITY: &str = r#"
    boundary operator == Float::equal(left: f32, right: f32) -> bool;
    boundary operator == Float::equal(left: f64, right: f64) -> bool;
"#;

#[test]
fn float_pattern_arms_select_exact_equality_overloads() {
    for (format, primitive) in [
        ("f32", typed_trees::types::PrimitiveType::F32),
        ("f64", typed_trees::types::PrimitiveType::F64),
    ] {
        let checked = check(&source(format, EQUALITY, "identity(first)"))
            .expect("each float arm uses the exact selected equality");
        let uses = checked
            .facts
            .operators
            .uses
            .iter()
            .map(|(_, operator_use)| operator_use)
            .filter(|operator_use| operator_use.spelling == OperatorSpelling::Equal)
            .collect::<Vec<_>>();
        assert_eq!(uses.len(), 2, "two value arms, no wildcard operator");
        for operator_use in uses {
            assert_eq!(
                operator_use.status,
                checked_trees::CheckedOperatorResolutionStatus::Resolved
            );
            let operator = checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
                .expect("selected exact declaration");
            for parameter in checked.typed.operator_parameters(operator) {
                assert_eq!(
                    checked
                        .typed
                        .primitive_type_reference(parameter.type_reference),
                    Some(primitive)
                );
            }
        }
        assert_eq!(
            checked.facts.operators.boundary_applications.len(),
            2,
            "equal overload identity does not merge authored applications"
        );
    }
}

#[test]
fn float_patterns_require_a_selected_equality() {
    for declarations in [
        "",
        "boundary operator == Float::equal(left: f64, right: f64) -> bool;",
    ] {
        let diagnostics = check(&source("f32", declarations, "identity(first)"))
            .expect_err("float comparisons have no implicit builtin provider");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("equality")
                    || diagnostic.message.contains("operator")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn ambiguous_float_pattern_equality_does_not_choose_a_provider() {
    let declarations =
        format!("{EQUALITY}\nboundary operator == Other::equal(left: f32, right: f32) -> bool;");
    let diagnostics = check(&source("f32", &declarations, "identity(first)"))
        .expect_err("two matching equality declarations are ambiguous");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ambiguous")),
        "{diagnostics:#?}"
    );
}

#[test]
fn selected_float_pattern_equality_must_return_boolean() {
    let declarations = "boundary operator == Float::equal(left: f32, right: f32) -> u64;";
    let diagnostics = check(&source("f32", declarations, "identity(first)"))
        .expect_err("a numeric result cannot select a match arm");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.to_lowercase().contains("bool")),
        "{diagnostics:#?}"
    );
}

#[test]
fn float_pattern_equality_requirements_are_not_skipped() {
    let declarations =
        "boundary operator == Float::equal(left: f32, right: f32) -> bool requires false;";
    let diagnostics = check(&source("f32", declarations, "identity(first)"))
        .expect_err("implicit comparisons owe the selected declaration requirements");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn anonymous_float_pattern_lands_at_the_selected_operand() {
    for format in ["f32", "f64"] {
        let checked = check(&source(format, EQUALITY, "1.5"))
            .expect("an anonymous numeric pattern lands to the float subject");
        assert_eq!(checked.facts.operators.boundary_applications.len(), 2);
    }
}

#[test]
fn implicit_equality_keeps_arm_identity_and_never_becomes_an_expression_operator() {
    let checked = check(&source("f32", EQUALITY, "identity(first)")).unwrap();
    let uses = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| operator_use)
        .filter(|operator_use| {
            matches!(
                operator_use.occurrence,
                checked_trees::CheckedOperatorOccurrence::MatchEquality { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 2);
    assert_eq!(uses[0].expression, uses[1].expression);
    assert_eq!(uses[0].origin, uses[1].origin);
    assert_ne!(uses[0].occurrence, uses[1].occurrence);
    assert_ne!(uses[0].application_site(), uses[1].application_site());
    assert!(
        checked
            .facts
            .operators
            .expression_use(uses[0].expression)
            .is_none()
    );
    for operator_use in uses {
        let operands = operator_use.operands(&checked.typed).unwrap();
        assert_eq!(operands.len(), 2);
        assert!(
            checked
                .facts
                .operators
                .boundary_applications
                .iter()
                .any(
                    |application| application.site == operator_use.application_site()
                        && application.requirement_symbol == operator_use.selected_operator_symbol
                )
        );
        let mut substituted = *operator_use;
        substituted.expression = operands[1];
        assert!(
            substituted.operands(&checked.typed).is_none(),
            "a pattern call is not its owning Match"
        );
        substituted = *operator_use;
        substituted.occurrence = checked_trees::CheckedOperatorOccurrence::MatchEquality {
            source_arm: arena::Handle::from_arena_index(u32::MAX),
        };
        assert!(
            substituted.operands(&checked.typed).is_none(),
            "an unrelated arm is not an operand source"
        );
        substituted = *operator_use;
        substituted.spelling = OperatorSpelling::NotEqual;
        assert!(
            substituted.operands(&checked.typed).is_none(),
            "value patterns select equality only"
        );
    }
}

#[test]
fn collecting_float_pattern_operators_does_not_manufacture_source_expressions() {
    use super::{Lexer, lower_symbol_resolved_trees, lower_syntax_trees, parse_syntax_trees};
    let source = source("f32", EQUALITY, "identity(first)");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let count = typed.expression_table.expression_nodes().count();
    let checked = crate::lower_typed_trees(typed).unwrap();
    assert_eq!(
        checked.typed.expression_table.expression_nodes().count(),
        count
    );
}

#[test]
fn selected_pattern_expression_and_implicit_comparison_have_separate_uses() {
    let declarations =
        format!("{EQUALITY}\nboundary operator + Float::add(left: f32, right: f32) -> f32;");
    let checked = check(&source("f32", &declarations, "first + second")).unwrap();
    let uses = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| operator_use)
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 3);
    assert_eq!(
        uses.iter()
            .filter(|operator_use| operator_use.spelling == OperatorSpelling::Equal)
            .count(),
        2
    );
    let addition = uses
        .iter()
        .find(|operator_use| operator_use.spelling == OperatorSpelling::Add)
        .unwrap();
    assert_eq!(
        addition.occurrence,
        checked_trees::CheckedOperatorOccurrence::Expression
    );
    assert!(
        checked
            .facts
            .operators
            .expression_use(addition.expression)
            .is_some()
    );
    assert_eq!(checked.facts.operators.boundary_applications.len(), 3);
}

#[test]
fn selected_float_equality_does_not_publish_builtin_match_propositions() {
    let checked = check(&format!("{EQUALITY}\nmachine choose(value: f32, pattern: f32) -> u64 {{ match value {{ pattern -> 7, _ -> 11 }} }}")).unwrap();
    assert!(
        !checked
            .facts
            .semantic
            .facts
            .iter()
            .any(|(_, fact)| matches!(fact.payload, facts::FactPayload::MatchPattern { .. })),
        "a selected equality contract is not builtin mathematical equality"
    );
    let integers = check(
        "machine choose(value: i64, pattern: i64) -> u64 { match value { pattern -> 7, _ -> 11 } }",
    )
    .unwrap();
    assert!(
        integers
            .facts
            .semantic
            .facts
            .iter()
            .any(|(_, fact)| matches!(fact.payload, facts::FactPayload::MatchPattern { .. })),
        "builtin scalar comparisons still contribute branch observations"
    );
}

#[test]
fn float_equality_requires_use_the_comparison_invocation_context() {
    let declarations = "boundary operator >= Float::at_least(left: f32, right: f32) -> bool;
        boundary operator == Float::equal(left: f32, right: f32) -> bool requires left >= 0.0f32;";
    let body = "{ match value { first -> 7, second -> 9, _ -> 11 } }";
    let checked = check(&format!(
        "{declarations}
        machine choose(value: f32, first: f32, second: f32) -> u64
        requires value >= 0.0f32 {body}"
    ))
    .expect("unchanged caller facts reach each comparison");
    assert_eq!(checked.facts.flow.control.operator_invocations.len(), 2);
    let diagnostics = check(&format!(
        "{declarations}
        machine choose(value: f32, first: f32, second: f32) -> u64 {body}"
    ))
    .expect_err("the same operands without proof do not discharge requires");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn crash_qualified_float_equality_requires_same_cause_ceiling() {
    let declarations =
        "boundary operator == Float::equal(left: f32, right: f32) -> bool crashes Trap;";
    let diagnostics = check(
        &source("f32", declarations, "identity(first)")
            .replace("machine choose(", "pub machine choose("),
    )
    .expect_err("a selected crash contract cannot disappear at an implicit comparison");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.to_lowercase().contains("crash")),
        "{diagnostics:#?}"
    );
}

#[test]
fn earlier_pattern_writes_remove_later_comparison_premises() {
    for (first_pattern, keeps_requirement) in
        [("self.pattern", true), ("reset(&mut self.pattern)", false)]
    {
        let checked = check(&format!(
            r#"
            {EQUALITY}
            boundary operator >= Float::at_least(left: f32, right: f32) -> bool;
            data Inputs {{ value: f32; pattern: f32; }}
            machine reset(value: &mut f32) -> f32 {{ value = -1.0f32; 0.0f32 }}
            machine Inputs::choose(&mut self) -> u64 requires self.pattern >= 0.0f32 {{
                match self.value {{ {first_pattern} -> 7, self.pattern -> 9, _ -> 11 }}
            }}
        "#
        ))
        .unwrap();
        let invocations = checked
            .facts
            .flow
            .control
            .operator_invocations
            .iter()
            .map(|(_, invocation)| invocation)
            .collect::<Vec<_>>();
        assert_eq!(invocations.len(), 2);
        let retained = checked
            .facts
            .flow
            .semantic_constraint_contexts(invocations[1].requires_constraints)
            .any(|context| {
                checked
                    .facts
                    .semantic
                    .context_view(checked.facts.semantic.contexts.get(context))
                    .facts()
                    .any(|fact| {
                        checked
                            .facts
                            .semantic
                            .boolean_fact_label(&checked.typed, fact)
                            .is_some_and(|label| {
                                label.contains("self.pattern") && label.contains(">=")
                            })
                    })
            });
        assert_eq!(retained, keeps_requirement, "{first_pattern}");
    }
}
