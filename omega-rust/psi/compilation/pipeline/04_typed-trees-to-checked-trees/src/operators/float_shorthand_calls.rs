//! Bind resolved float shorthand to the ordinary requirement-call route.
//!
//! Retaining a selected `F64::minimum` use beside a call to the builtin `min`
//! symbol is not executable supply: ordinary call checking needs the selected
//! declaration's signature, effects and source identity. Normalize that join
//! before validation, including calls synthesized by `abs` and `clamp`. This
//! selects a requirement, never a provider; Omega still installs and replays
//! its selected implementation. Integer intrinsics and authored same-spelled
//! functions are not rewritten.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(super) fn bind(
    program: &mut TypedTrees,
    facts: &checked_trees::CheckedOperatorFacts,
    requirements: &arena::Arena<checked_trees::CheckedNamedRequirementUseFact>,
) -> Result<usize, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    for (expression, selected) in facts
        .named_uses()
        .map(|use_fact| (use_fact.expression, use_fact.selected_operator_symbol))
        .chain(
            requirements
                .iter()
                .map(|(_, use_fact)| (use_fact.expression, use_fact.requirement_symbol)),
        )
    {
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            continue;
        };
        if !matches!(
            program
                .symbols
                .builtin_function_for_symbol(call.target_symbol),
            Some(
                symbols::BuiltinFunction::Min
                    | symbols::BuiltinFunction::Max
                    | symbols::BuiltinFunction::Sqrt
            )
        ) || call.receiver.is_valid()
            || !call.carries_only_positional_arguments()
            || !call.selects_only_nominal_route()
        {
            continue;
        }
        let target = program
            .machine_by_symbol(selected)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .first()
                    .map(|entry| entry.symbol)
            })
            .unwrap_or(selected);
        if let Some((_, previous)) = bindings.iter().find(|(bound, _)| *bound == expression) {
            if *previous != target {
                return Err(vec![Diagnostic::error(
                    "a float shorthand occurrence selects conflicting requirement identities",
                )]);
            }
        } else {
            bindings.push((expression, target));
        }
    }
    for &(expression, target) in &bindings {
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) {
            call.target_symbol = target;
        }
    }
    Ok(bindings.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::front_end::typed_program;

    #[test]
    fn shorthand_keeps_its_occurrence_and_operands_and_binds_once() {
        let mut program = typed_program(
            r#"
            pub data F32 {}
            pub boundary operator F32::minimum(left: f32, right: f32) -> f32;
            machine value(left: f32, right: f32) -> f32 { min(left, right) }
        "#,
        );
        let values = crate::values::build_value_origin_rows(&program);
        let facts = super::super::build_operator_uses(&program, &values);
        let selected = facts.named_uses().next().expect("selected shorthand");
        let expression = selected.expression;
        let requirement = selected.selected_operator_symbol;
        let original = program.expression_table.expression(expression).clone();
        let ExpressionNode::Call(mut expected) = original else {
            panic!("call")
        };
        assert_eq!(
            program
                .symbols
                .builtin_function_for_symbol(expected.target_symbol),
            Some(symbols::BuiltinFunction::Min)
        );
        expected.target_symbol = requirement;
        assert_eq!(
            super::super::bind_selected_operator_calls(&mut program).unwrap(),
            1
        );
        assert_eq!(
            program.expression_table.expression(expression),
            &ExpressionNode::Call(expected)
        );
        assert_eq!(
            super::super::bind_selected_operator_calls(&mut program).unwrap(),
            0
        );
    }

    #[test]
    fn integer_intrinsics_and_authored_functions_keep_their_targets() {
        for source in [
            "machine value(left: i32, right: i32) -> i32 { min(left, right) }",
            "machine min(left: f64, right: f64) -> f64 { left } machine value(left: f64, right: f64) -> f64 { min(left, right) }",
            "pub data F32 {} pub boundary operator F32::minimum(left: f64, right: f64) -> f64; machine value(left: f32, right: f32) -> f32 { min(left, right) }",
        ] {
            let mut program = typed_program(source);
            assert_eq!(
                super::super::bind_selected_operator_calls(&mut program).unwrap(),
                0,
                "{source}"
            );
        }
    }

    #[test]
    fn checked_shorthand_retains_the_selected_call_declaration() {
        for source in [
            "pub data F32 {} pub boundary operator F32::minimum(left: f32, right: f32) -> f32; machine value(left: f32, right: f32) -> f32 { min(left, right) }",
            "machine value(left: i32, right: i32) -> i32 { min(left, right) }",
            "machine min(left: f64, right: f64) -> f64 { left } machine value(left: f64, right: f64) -> f64 { min(left, right) }",
        ] {
            let checked = crate::tests::front_end::checked_program(source);
            assert!(checked.authored_declaration_selections().all_finalized());
            let mut selected_calls = 0;
            for (expression, node) in checked.expression_table.iter_expressions() {
                let ExpressionNode::Call(call) = node else {
                    continue;
                };
                for occurrence in checked
                    .expression_table
                    .authored_selection_occurrences(expression)
                {
                    let selection = checked
                        .authored_declaration_selections()
                        .get(occurrence)
                        .unwrap();
                    if selection.kind() != typed_trees::AuthoredDeclarationSelectionKind::Call {
                        continue;
                    }
                    let typed_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
                        selection.target()
                    else {
                        panic!("expected selected call declaration: {selection:?}");
                    };
                    assert_eq!(target.selected_symbol(), call.target_symbol, "{source}");
                    selected_calls += 1;
                }
            }
            assert_eq!(selected_calls, 1, "{source}");
        }
    }

    /// `abs` and `clamp` lower to `max`/`min` applications of their one
    /// authored call. Each application keeps its own authored Call selection,
    /// settled on the declaration it executes, so review custody can rejoin a
    /// bound `F64::maximum` exactly as it rejoins an authored `max`.
    #[test]
    fn abs_and_clamp_applications_keep_one_exact_call_selection_each() {
        let float_requirements = "pub data F64 {}
            pub boundary machine - Float::subtract(left: f64, right: f64) -> f64;
            pub boundary operator F64::minimum(left: f64, right: f64) -> f64;
            pub boundary operator F64::maximum(left: f64, right: f64) -> f64;";
        for (source, applications) in [
            (
                format!("{float_requirements} machine value(input: f64) -> f64 {{ abs(input) }}"),
                1,
            ),
            (
                format!(
                    "{float_requirements} machine value(input: f64) -> f64 {{ clamp(input, 0.0, 25.0) }}"
                ),
                2,
            ),
            (
                "machine value(input: i32) -> i32 { clamp(input, 0, 25) }".to_owned(),
                2,
            ),
        ] {
            let checked = crate::tests::front_end::checked_program(&source);
            assert!(checked.authored_declaration_selections().all_finalized());
            let mut selected = Vec::new();
            for (expression, node) in checked.expression_table.iter_expressions() {
                // Token-bound operator calls (the float `0 - input` of `abs`)
                // keep their authored Operator selection instead.
                let ExpressionNode::Call(call) = node else {
                    continue;
                };
                if call.operational_acknowledgement.origin
                    == language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
                {
                    continue;
                }
                let calls = checked
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .filter_map(|occurrence| {
                        checked.authored_declaration_selections().get(occurrence)
                    })
                    .filter(|selection| {
                        selection.kind() == typed_trees::AuthoredDeclarationSelectionKind::Call
                    })
                    .collect::<Vec<_>>();
                let [selection] = calls.as_slice() else {
                    panic!(
                        "{source}: {} Call selections on one application",
                        calls.len()
                    );
                };
                let typed_trees::AuthoredDeclarationSelectionTarget::Resolved(target) =
                    selection.target()
                else {
                    panic!("{source}: unsettled selection {selection:?}");
                };
                assert_eq!(target.selected_symbol(), call.target_symbol, "{source}");
                selected.push(selection.source_span());
            }
            assert_eq!(selected.len(), applications, "{source}");
            let mut distinct = Vec::new();
            for span in selected {
                if !distinct.contains(&span) {
                    distinct.push(span);
                }
            }
            assert_eq!(
                distinct.len(),
                applications,
                "{source}: distinct occurrences"
            );
        }
    }

    #[test]
    fn shorthand_still_owes_the_selected_requirements() {
        for declaration in ["operator", "requirement"] {
            let source = format!(
                r#"
            pub data F64 {{}}
            pub boundary {declaration} F64::minimum(left: f64, right: f64) -> f64
                requires false;
            machine value(left: f64, right: f64) -> f64 {{ min(left, right) }}
        "#,
            );
            let diagnostics = crate::tests::front_end::checked_program_result(&source)
                .expect_err("normalization must not bypass the selected contract");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("requires")),
                "{declaration}: {diagnostics:?}"
            );
        }
    }
}
