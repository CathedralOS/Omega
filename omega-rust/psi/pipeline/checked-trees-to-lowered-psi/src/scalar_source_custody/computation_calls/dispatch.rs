//! Independently reconstruct the exact covered arm prefix from authored syntax.

use super::*;
use checked_trees::expression::MatchPattern;
use checked_trees::{CheckedScalarDispatchArm, CheckedScalarDispatchPattern};

pub(super) fn source_scope(
    checked: &CheckedTrees,
    mut scope: ExpressionHandle,
    source: ExpressionHandle,
    result_type: PrimitiveType,
) -> Result<(), LoweringError> {
    if !authored_expressions(checked, scope)?.contains(&source) {
        return unsupported("computed dispatch escaped its authored operand scope");
    }
    while scope != source {
        let selected = operand_scopes::folded_match_scope(checked, scope)?;
        if selected != scope {
            scope = selected;
            continue;
        }
        if let ExpressionNode::Cast(cast) = checked.expression_table.expression(scope)
            && cast.semantic_domain.is_empty()
            && checked.primitive_type_reference(cast.target_type) == Some(result_type)
        {
            scope = cast.value;
            continue;
        }
        let (condition, selected, _) = operand_scopes::selection(checked, scope)?;
        scope = if authored_expressions(checked, condition)?.contains(&source) {
            condition
        } else {
            selected
        };
    }
    Ok(())
}

pub(super) fn operands(
    checked: &CheckedTrees,
    source: ExpressionHandle,
    subject: CheckedScalarComputationHandle,
    arms: arena::HandleSpan<CheckedScalarDispatchArm>,
    result_type: PrimitiveType,
) -> Result<Vec<(CheckedScalarComputationHandle, ExpressionHandle)>, LoweringError> {
    let ExpressionNode::Match(dispatch) = checked.expression_table.expression(source) else {
        return unsupported("computed dispatch lost its authored match occurrence");
    };
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(subject) {
        return unsupported("computed dispatch has no live subject");
    }
    let subject_type = plans.nodes.get(subject).primitive_type;
    if validation::match_subject_primitive_type(&checked.typed, dispatch)
        .is_some_and(|expected| expected != subject_type)
    {
        return unsupported("computed dispatch substituted its authored subject carrier");
    }
    let authored = checked.expression_table.match_arms(dispatch.arms);
    if authored.len() != dispatch.arms.len() {
        return unsupported("computed dispatch has stale authored alternatives");
    }
    let retained = plans
        .dispatch_arms
        .span(arms)
        .ok_or(LoweringError::Unsupported(
            "computed dispatch has stale retained alternatives",
        ))?;
    let mut operands = vec![(subject, dispatch.subject)];
    let mut boolean_coverage = [false; 2];
    let mut covered = false;
    for (ordinal, arm) in retained.iter().enumerate() {
        let authored_arm = authored.get(ordinal).ok_or(LoweringError::Unsupported(
            "computed dispatch added an unauthored alternative",
        ))?;
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| LoweringError::Unsupported("computed dispatch arm ordinal overflows"))?;
        let source_arm = arena::Handle::from_parts(
            dispatch
                .arms
                .start()
                .arena_index()
                .checked_add(ordinal)
                .ok_or(LoweringError::Unsupported(
                    "computed dispatch arm identity overflows",
                ))?,
            dispatch.arms.start().generation(),
        );
        if covered
            || arm.source_arm != source_arm
            || !plans.nodes.is_valid(arm.value)
            || plans.nodes.get(arm.value).primitive_type != result_type
        {
            return unsupported("computed dispatch substituted its ordered arm identity or result");
        }
        match (&arm.pattern, &authored_arm.pattern) {
            (CheckedScalarDispatchPattern::Wildcard, MatchPattern::Wildcard) => covered = true,
            (CheckedScalarDispatchPattern::Value(pattern), MatchPattern::Value(expression)) => {
                if !plans.nodes.is_valid(*pattern)
                    || plans.nodes.get(*pattern).primitive_type != subject_type
                {
                    return unsupported(
                        "computed dispatch pattern carrier disagrees with its subject",
                    );
                }
                operands.push((*pattern, *expression));
                if subject_type == PrimitiveType::Bool
                    && let ExpressionNode::Boolean(value) =
                        checked.expression_table.expression(*expression)
                {
                    if !matches!(&plans.nodes.get(*pattern).kind,
                        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(retained))
                        if matches!(retained.as_ref(), CheckedBooleanExpression::Constant(retained) if retained == value))
                    {
                        return unsupported(
                            "computed dispatch changed a coverage literal's meaning",
                        );
                    }
                    boolean_coverage[usize::from(*value)] = true;
                    covered = boolean_coverage.iter().all(|value| *value);
                }
            }
            _ => return unsupported("computed dispatch substituted its authored pattern"),
        }
        operands.push((arm.value, authored_arm.value));
    }
    if !covered {
        return unsupported("computed dispatch omitted required coverage alternatives");
    }
    Ok(operands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_dispatch_rejects_operations_from_an_unselected_arm() {
        for (carrier, selected, skipped) in [
            ("u64", "identity(7) | 8", "identity(9) | 8"),
            ("bool", "identity(true) && true", "identity(false) && true"),
        ] {
            let source = format!(
                "machine identity(value: {carrier}) -> {carrier} {{ value }}
                 machine choose() -> {carrier} {{ match 1 {{ 1 -> {selected}, _ -> {skipped} }} }}"
            );
            let tokens = source_files_to_tokens::Lexer::new(&source)
                .tokenize()
                .unwrap();
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
            let resolved =
                syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
            let typed =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .unwrap();
            let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
            let root = checked
                .facts
                .values
                .scalar_computations
                .roots
                .iter()
                .map(|(_, root)| root.clone())
                .find(|root| {
                    matches!(
                        checked.expression_table.expression(
                            checked
                                .facts
                                .values
                                .scalar_computations
                                .nodes
                                .get(root.root)
                                .authored_root
                        ),
                        ExpressionNode::Match(_)
                    )
                })
                .expect("folded match root");
            let authored = checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(root.root)
                .authored_root;
            let ExpressionNode::Match(dispatch) = checked.expression_table.expression(authored)
            else {
                panic!("authored match");
            };
            let skipped = checked.expression_table.match_arms(dispatch.arms)[1].value;
            let validate = |checked: &CheckedTrees| {
                validate_computation_calls(
                    checked,
                    root.machine,
                    root.state,
                    root.statement_ordinal,
                    root.root,
                    authored,
                )
            };
            validate(&checked).expect("selected operation source custody");
            match &mut checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(root.root)
                .kind
            {
                CheckedScalarComputationKind::Apply {
                    source_expression, ..
                }
                | CheckedScalarComputationKind::Select {
                    source_expression, ..
                } => {
                    *source_expression = skipped;
                }
                _ => panic!("retained call-bearing application or selection"),
            }
            assert!(
                validate(&checked).is_err(),
                "dead operation is not an executable descendant"
            );
        }
    }

    #[test]
    fn folded_dispatch_replay_rejects_changed_selection_and_subject_meaning() {
        let source = "machine identity(value: u64) -> u64 { value }
            machine choose() -> u64 {
                match 18446744073709551616 / 18446744073709551616 {
                    1 -> identity(7), _ -> identity(9)
                }
            }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root.clone())
            .find(|root| {
                matches!(
                    checked.expression_table.expression(
                        checked
                            .facts
                            .values
                            .scalar_computations
                            .nodes
                            .get(root.root)
                            .authored_root
                    ),
                    ExpressionNode::Match(_)
                )
            })
            .expect("folded match root");
        let authored = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root.root)
            .authored_root;
        let ExpressionNode::Match(dispatch) = checked.expression_table.expression(authored).clone()
        else {
            panic!("authored match");
        };
        let validate = |checked: &CheckedTrees| {
            validate_computation_calls(
                checked,
                root.machine,
                root.state,
                root.statement_ordinal,
                root.root,
                authored,
            )
        };
        validate(&checked).expect("folded call retains source custody");
        let mut arms = checked.expression_table.match_arms(dispatch.arms).to_vec();
        let original_arms = dispatch.arms;
        let original_selected = arms[0].value;
        arms[0].value = arms[1].value;
        let replaced_arms = checked.typed.expression_table.insert_match_arms(arms);
        let ExpressionNode::Match(replaced) =
            checked.typed.expression_table.expression_mut(authored)
        else {
            panic!("authored match");
        };
        replaced.arms = replaced_arms;
        assert!(
            validate(&checked).is_err(),
            "changing selected body invalidates retained call"
        );
        let ExpressionNode::Match(replaced) =
            checked.typed.expression_table.expression_mut(authored)
        else {
            panic!("authored match");
        };
        replaced.arms = original_arms;

        let original_subject = checked
            .expression_table
            .expression(dispatch.subject)
            .clone();
        *checked
            .typed
            .expression_table
            .expression_mut(dispatch.subject) = ExpressionNode::Boolean(true);
        assert!(
            validate(&checked).is_err(),
            "changed subject type cannot retain anonymous folding"
        );
        *checked
            .typed
            .expression_table
            .expression_mut(dispatch.subject) = checked
            .expression_table
            .expression(original_selected)
            .clone();
        assert!(
            validate(&checked).is_err(),
            "effectful subject cannot be skipped"
        );
        *checked
            .typed
            .expression_table
            .expression_mut(dispatch.subject) = original_subject;
        validate(&checked).expect("restored source custody");

        assert!(
            checked
                .facts
                .operators
                .expression_use(dispatch.subject)
                .is_none()
        );
        checked
            .facts
            .operators
            .uses
            .insert(checked_trees::CheckedOperatorUseFact {
                expression: dispatch.subject,
                status: checked_trees::CheckedOperatorResolutionStatus::Inadmissible,
                ..Default::default()
            });
        assert!(
            validate(&checked).is_err(),
            "changed operator meaning cannot retain folded control"
        );
    }

    #[test]
    fn dispatch_replay_rejects_swapped_results_and_missing_coverage() {
        let source = "machine choose(subject: bool) -> bool {
            match subject { true -> false, false -> true }
        }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root.root);
        let authored = node.authored_root;
        let CheckedScalarComputationKind::Dispatch { arms, .. } = node.kind else {
            panic!("retained dispatch");
        };
        let validate = |checked: &CheckedTrees| {
            validate_computation_calls(
                checked,
                root.machine,
                root.state,
                root.statement_ordinal,
                root.root,
                authored,
            )
        };
        validate(&checked).expect("original dispatch source correspondence");
        let original = checked
            .facts
            .values
            .scalar_computations
            .dispatch_arms
            .span(arms)
            .unwrap()
            .to_vec();
        let retained = checked
            .facts
            .values
            .scalar_computations
            .dispatch_arms
            .span_mut(arms)
            .unwrap();
        retained[0].value = original[1].value;
        retained[1].value = original[0].value;
        assert!(
            validate(&checked).is_err(),
            "same-typed arm results cannot exchange source occurrences"
        );
        checked
            .facts
            .values
            .scalar_computations
            .dispatch_arms
            .span_mut(arms)
            .unwrap()
            .clone_from_slice(&original);
        let CheckedScalarComputationKind::Dispatch { arms, .. } = &mut checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(root.root)
            .kind
        else {
            panic!("retained dispatch");
        };
        *arms = arena::HandleSpan::from_parts(arms.start(), 1);
        assert!(
            validate(&checked).is_err(),
            "a final arbitrary value arm is not default coverage"
        );
    }
}
