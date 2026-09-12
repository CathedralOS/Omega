//! Reconstruct fresh value ownership and ordered selection from authored nodes.

use crate::{LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use checked_trees::statement::StatementNode;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarDispatchPattern, CheckedScalarExpressionRole,
    CheckedStructuralValueKind, CheckedTrees, CheckedUnitEffectOperationPlan,
};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

pub(crate) fn validate(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } = operation
    else {
        return unsupported("structural construction operation absent");
    };
    let (owner, source) = crate::scalar_source_custody::authored_state(checked, state)?;
    let (expression, reference) = match checked
        .statement_table
        .statements(source.statement_nodes)
        .get(result.statement_index as usize)
    {
        Some(StatementNode::LocalData(local))
            if !local.is_mutable && local.initial_value.is_valid() =>
        {
            (local.initial_value, local.type_reference)
        }
        Some(StatementNode::Expression(expression)) => (*expression, source.return_type),
        _ => return unsupported("structural construction lost its authored destination"),
    };
    let plans = &checked.facts.values.structural_values;
    let root = plans
        .root_at(state, result.statement_index)
        .ok_or(LoweringError::Unsupported(
            "structural construction has no unique source root",
        ))?;
    if owner.symbol != machine
        || root.machine != machine
        || root.state != state
        || root.expression != expression
        || root.type_reference != reference
        || root.root != *value
        || checked.normalized_type_identity(reference).as_str() != result.type_identity
        || checked.type_multiplicity(reference) != result.multiplicity
        || !validation::has_plain_owned_contents_with_numeric_constraints(&checked.typed, reference)
    {
        return unsupported("structural construction substituted its owner or result type");
    }
    let mut pending = vec![(*value, expression)];
    let mut visited = Vec::new();
    let mut operand_roles = Vec::new();
    while let Some((handle, expression)) = pending.pop() {
        if !plans.nodes.is_valid(handle) || visited.contains(&handle) {
            return unsupported("structural construction has stale or reused value nodes");
        }
        visited.push(handle);
        let node = plans.nodes.get(handle);
        if node.expression != expression {
            return unsupported("structural construction exchanged authored value occurrences");
        }
        match node.kind {
            CheckedStructuralValueKind::Case {
                data_symbol,
                case_symbol,
            } => {
                if validation::fresh_payloadless_case(&checked.typed, expression, reference)
                    != Some((data_symbol, case_symbol))
                {
                    return unsupported("structural construction substituted its exact fresh case");
                }
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                let ExpressionNode::Match(dispatch) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("structural selection has no authored match");
                };
                let primitive = validate_operand(
                    checked,
                    machine,
                    state,
                    result.statement_index,
                    CheckedScalarExpressionRole::StructuralValueSubject { expression },
                    subject,
                    dispatch.subject,
                )?;
                operand_roles
                    .push(CheckedScalarExpressionRole::StructuralValueSubject { expression });
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
                for (ordinal, arm) in retained.iter().enumerate() {
                    let source_arm = dispatch
                        .arms
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("structural arm ordinal overflow")
                        })?)
                        .map(|index| {
                            arena::Handle::from_parts(index, dispatch.arms.start().generation())
                        })
                        .ok_or(LoweringError::Unsupported(
                            "structural arm identity overflow",
                        ))?;
                    let authored_arm = authored.get(ordinal).ok_or(LoweringError::Unsupported(
                        "structural selection added an unauthored alternative",
                    ))?;
                    if covered || arm.source_arm != source_arm {
                        return unsupported(
                            "structural selection reordered its covered alternatives",
                        );
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
                            operand_roles.push(
                                CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                            );
                            if validate_operand(
                                checked,
                                machine,
                                state,
                                result.statement_index,
                                CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                                *pattern,
                                *authored_pattern,
                            )? != primitive
                            {
                                return unsupported(
                                    "structural pattern changed its subject carrier",
                                );
                            }
                            if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                                let occurrence =
                                    crate::scalar_computations::comparisons::occurrence(
                                        checked,
                                        arm.equality_use,
                                        machine,
                                        state,
                                        result.statement_index,
                                    )?;
                                let selected = checked.facts.operators.uses.get(arm.equality_use);
                                if selected.expression != expression || selected.occurrence != (checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm }) || occurrence.comparison != semantic_vocabulary::IeeeFloatComparisonOperation::Equal {
                                    return unsupported("structural pattern substituted selected equality");
                                }
                            } else if arm.equality_use.is_valid() {
                                return unsupported(
                                    "structural builtin pattern acquired selected equality",
                                );
                            }
                            if primitive == PrimitiveType::Bool
                                && let ExpressionNode::Boolean(value) =
                                    checked.expression_table.expression(*authored_pattern)
                            {
                                if !matches!(&checked.facts.values.scalar_computations.nodes.get(*pattern).kind,
                                    checked_trees::CheckedScalarComputationKind::Value(checked_trees::CheckedScalarExpression::Boolean(retained))
                                        if matches!(retained.as_ref(), checked_trees::CheckedBooleanExpression::Constant(retained) if retained == value))
                                {
                                    return unsupported(
                                        "structural selection changed a coverage literal",
                                    );
                                }
                                booleans[usize::from(*value)] = true;
                                covered = booleans.iter().all(|value| *value);
                            }
                        }
                        _ => {
                            return unsupported(
                                "structural selection substituted its authored pattern",
                            );
                        }
                    }
                    pending.push((arm.value, authored_arm.value));
                }
                if !covered {
                    return unsupported("structural selection omitted required coverage");
                }
            }
        }
    }
    if checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .any(|root| {
            root.state == state
                && root.statement_ordinal == result.statement_index
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::StructuralValueSubject { .. }
                        | CheckedScalarExpressionRole::StructuralValuePattern { .. }
                )
                && !operand_roles.contains(&root.role)
        })
    {
        return unsupported("structural construction acquired an unauthored scalar root");
    }
    Ok(())
}

fn validate_operand(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
    handle: CheckedScalarComputationHandle,
    expression: ExpressionHandle,
) -> Result<PrimitiveType, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == statement && root.role == role
    });
    let root = roots.next().ok_or(LoweringError::Unsupported(
        "structural operand lost its computation root",
    ))?;
    if roots.next().is_some()
        || root.machine != machine
        || root.root != handle
        || !plans.nodes.is_valid(handle)
        || plans.nodes.get(handle).authored_root != expression
    {
        return unsupported("structural operand substituted its source computation");
    }
    crate::scalar_source_custody::validate_computation_calls(
        checked, machine, state, statement, handle, expression,
    )?;
    crate::scalar_source_custody::value_correspondence::validate(
        checked,
        state,
        statement,
        expression,
        plans.nodes.get(handle).primitive_type,
        &checked_trees::CheckedCallScalarArgument::Computation(handle),
    )?;
    Ok(plans.nodes.get(handle).primitive_type)
}

/// Locate only operands of actual match nodes beneath this statement's value.
/// A retained role cannot authorize reading an unrelated expression in the arena.
pub(crate) fn operand_source(
    checked: &CheckedTrees,
    state: SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
) -> Result<(ExpressionHandle, PrimitiveType), LoweringError> {
    let (machine, source) = crate::scalar_source_custody::authored_state(checked, state)?;
    let expression = match checked
        .statement_table
        .statements(source.statement_nodes)
        .get(statement as usize)
    {
        Some(StatementNode::LocalData(local)) if !local.is_mutable => local.initial_value,
        Some(StatementNode::Expression(expression)) => *expression,
        _ => return unsupported("structural operand has no authored value scope"),
    };
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            return unsupported("structural operand has cyclic authored scope");
        }
        visited.push(expression);
        let ExpressionNode::Match(dispatch) = checked.expression_table.expression(expression)
        else {
            continue;
        };
        let primitive = validation::expression_result_type_reference(
            &checked.typed,
            machine,
            source,
            dispatch.subject,
        )
        .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference))
        .and_then(|reference| checked.primitive_type_reference(reference))
        .or_else(|| validation::match_subject_primitive_type(&checked.typed, dispatch));
        if role == (CheckedScalarExpressionRole::StructuralValueSubject { expression }) {
            return primitive
                .map(|primitive| (dispatch.subject, primitive))
                .ok_or(LoweringError::Unsupported(
                    "structural subject has no scalar carrier",
                ));
        }
        for (ordinal, arm) in checked
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let source_arm = arena::Handle::from_parts(
                dispatch
                    .arms
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("structural arm ordinal overflow")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural arm identity overflow",
                    ))?,
                dispatch.arms.start().generation(),
            );
            if role == (CheckedScalarExpressionRole::StructuralValuePattern { source_arm })
                && let MatchPattern::Value(pattern) = arm.pattern
            {
                return primitive.map(|primitive| (pattern, primitive)).ok_or(
                    LoweringError::Unsupported("structural pattern has no scalar carrier"),
                );
            }
            pending.push(arm.value);
        }
    }
    unsupported("structural operand role escaped its authored value scope")
}
