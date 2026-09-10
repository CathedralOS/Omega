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
        if let ExpressionNode::Cast(cast) = checked.expression_table.expression(scope)
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
