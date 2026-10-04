//! Independently reconstruct the exact covered arm prefix from authored syntax.
use super::{
    CheckedBooleanExpression, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarExpression, CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError,
    PrimitiveType, authored_expressions, operand_scopes, unsupported,
};
use checked_trees::expression::MatchPattern;
use checked_trees::{CheckedScalarDispatchArm, CheckedScalarDispatchPattern};

pub(super) fn source_scope(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
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
            && !operand_scopes::cast_requires_custody(checked, machine, state, cast)?
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
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
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
            (CheckedScalarDispatchPattern::Wildcard, MatchPattern::Wildcard) => {
                if arm.equality_use.is_valid() {
                    return unsupported("wildcard retained an unauthored comparison");
                }
                covered = true;
            }
            (CheckedScalarDispatchPattern::Value(pattern), MatchPattern::Value(expression)) => {
                if matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64) {
                    let occurrence =
                        crate::expression_preparation::source_custody::comparisons::occurrence(
                            checked,
                            arm.equality_use,
                            machine,
                            state,
                            statement,
                        )?;
                    let selected = checked.facts.operators.uses.get(arm.equality_use);
                    if selected.expression != source
                        || selected.occurrence
                            != (checked_trees::CheckedOperatorOccurrence::MatchEquality {
                                source_arm,
                            })
                        || !matches!(
                            occurrence.meaning,
                            crate::emission::selected_comparison::SelectedComparisonMeaning::IeeeFloat {
                                comparison: semantic_vocabulary::IeeeFloatComparisonOperation::Equal,
                                ..
                            }
                        )
                    {
                        return unsupported(
                            "dispatch comparison substituted its authored arm meaning",
                        );
                    }
                } else if arm.equality_use.is_valid() {
                    return unsupported("builtin dispatch acquired selected float equality");
                }
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
