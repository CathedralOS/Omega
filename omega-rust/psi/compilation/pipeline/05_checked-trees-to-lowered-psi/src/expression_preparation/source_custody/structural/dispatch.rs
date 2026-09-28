//! A structural selection replays against its authored `match`: the subject
//! keeps its scalar carrier, the retained arms cover the authored arms in
//! order, each value pattern keeps its operand role and selected equality,
//! and each arm's value is queued for replay under that arm.

use super::{
    CheckedScalarComputationHandle, CheckedScalarDispatchPattern, CheckedScalarExpressionRole,
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, MatchPattern, PrimitiveType,
    Replay, TypeReferenceHandle, unsupported, validate_operand,
};
use arena::HandleSpan;
use checked_trees::CheckedStructuralDispatchArm;

/// Replay one `Dispatch` node established at `expression` for `reference`.
pub(super) fn validate(
    checked: &CheckedTrees,
    replay: &mut Replay<'_>,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    subject: CheckedScalarComputationHandle,
    arms: HandleSpan<CheckedStructuralDispatchArm>,
) -> Result<(), LoweringError> {
    let Replay {
        machine,
        state,
        statement_index,
        pending,
        operand_roles,
    } = replay;
    let (machine, state, statement_index) = (*machine, *state, *statement_index);
    let plans = &checked.facts.values.structural_values;
    let ExpressionNode::Match(dispatch) = checked.expression_table.expression(expression) else {
        return unsupported("structural selection has no authored match");
    };
    let primitive = validate_operand(
        checked,
        machine,
        state,
        statement_index,
        CheckedScalarExpressionRole::StructuralValueSubject { expression },
        subject,
        dispatch.subject,
    )?;
    operand_roles.push(CheckedScalarExpressionRole::StructuralValueSubject { expression });
    if validation::match_subject_primitive_type(&checked.typed, dispatch)
        .is_some_and(|expected| expected != primitive)
    {
        return unsupported("structural selection changed its subject carrier");
    }
    let authored = checked.expression_table.match_arms(dispatch.arms);
    if authored.len() != dispatch.arms.len() {
        return unsupported("structural selection has stale source alternatives");
    }
    let retained = plans
        .dispatch_arms
        .span(arms)
        .ok_or(LoweringError::Unsupported(
            "structural selection has stale alternatives",
        ))?;
    let mut covered = false;
    let mut booleans = [false; 2];
    let mut retained_ordinal = 0;
    for (ordinal, authored_arm) in authored.iter().enumerate() {
        if covered {
            break;
        }
        if primitive == PrimitiveType::Bool
            && let MatchPattern::Value(pattern) = authored_arm.pattern
            && let ExpressionNode::Boolean(value) = checked.expression_table.expression(pattern)
            && booleans[usize::from(*value)]
        {
            continue;
        }
        let arm = retained
            .get(retained_ordinal)
            .ok_or(LoweringError::Unsupported(
                "structural selection omitted an executable alternative",
            ))?;
        retained_ordinal += 1;
        let source_arm = dispatch
            .arms
            .start()
            .arena_index()
            .checked_add(
                u32::try_from(ordinal)
                    .map_err(|_| LoweringError::Unsupported("structural arm ordinal overflow"))?,
            )
            .map(|index| arena::Handle::from_parts(index, dispatch.arms.start().generation()))
            .ok_or(LoweringError::Unsupported(
                "structural arm identity overflow",
            ))?;
        if covered || arm.source_arm != source_arm {
            return unsupported("structural selection reordered its covered alternatives");
        }
        match (&arm.pattern, &authored_arm.pattern) {
            (CheckedScalarDispatchPattern::Wildcard, MatchPattern::Wildcard) => {
                if arm.equality_use.is_valid() {
                    return unsupported("structural wildcard acquired a comparison");
                }
                covered = true;
            }
            (
                CheckedScalarDispatchPattern::Value(pattern),
                MatchPattern::Value(authored_pattern),
            ) => {
                operand_roles
                    .push(CheckedScalarExpressionRole::StructuralValuePattern { source_arm });
                if validate_operand(
                    checked,
                    machine,
                    state,
                    statement_index,
                    CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                    *pattern,
                    *authored_pattern,
                )? != primitive
                {
                    return unsupported("structural pattern changed its subject carrier");
                }
                if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                    let occurrence =
                        crate::expression_preparation::source_custody::comparisons::occurrence(
                            checked,
                            arm.equality_use,
                            machine,
                            state,
                            statement_index,
                        )?;
                    let selected = checked.facts.operators.uses.get(arm.equality_use);
                    if selected.expression != expression || selected.occurrence != (checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm }) || !matches!(occurrence.meaning, crate::emission::selected_comparison::SelectedComparisonMeaning::IeeeFloat { comparison: semantic_vocabulary::IeeeFloatComparisonOperation::Equal, .. }) {
                        return unsupported("structural pattern substituted selected equality");
                    }
                } else if arm.equality_use.is_valid() {
                    return unsupported("structural builtin pattern acquired selected equality");
                }
                if primitive == PrimitiveType::Bool
                    && let ExpressionNode::Boolean(value) =
                        checked.expression_table.expression(*authored_pattern)
                {
                    if !matches!(&checked.facts.values.scalar_computations.nodes.get(*pattern).kind,
                        checked_trees::CheckedScalarComputationKind::Value(checked_trees::CheckedScalarExpression::Boolean(retained))
                            if matches!(retained.as_ref(), checked_trees::CheckedBooleanExpression::Constant(retained) if retained == value))
                    {
                        return unsupported("structural selection changed a coverage literal");
                    }
                    booleans[usize::from(*value)] = true;
                    covered = booleans.iter().all(|value| *value);
                }
            }
            _ => {
                return unsupported("structural selection substituted its authored pattern");
            }
        }
        pending.push((
            arm.value,
            authored_arm.value,
            reference,
            arm.source_arm,
            None,
        ));
    }
    if !covered || retained_ordinal != retained.len() {
        return unsupported("structural selection omitted required coverage");
    }
    Ok(())
}
