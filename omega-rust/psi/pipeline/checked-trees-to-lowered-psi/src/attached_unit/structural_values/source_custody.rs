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
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        discard_result_on_return,
    } = operation
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
    if let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(result.statement_index as usize)
    {
        validate_local_ownership(
            checked,
            machine,
            state,
            result.statement_index,
            local.symbol,
            result.multiplicity,
            *discard_result_on_return,
        )?;
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
        match node.kind.clone() {
            CheckedStructuralValueKind::Record {
                data_symbol,
                fields,
            } => {
                let ExpressionNode::StructLiteral(literal) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("record construction lost its literal");
                };
                let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("record construction lost its nominal type");
                };
                let record = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == data_symbol)
                    .ok_or(LoweringError::Unsupported("record declaration missing"))?;
                let authored = checked.expression_table.struct_fields(literal.fields);
                let retained = plans
                    .record_fields
                    .span(fields)
                    .ok_or(LoweringError::Unsupported("record field span is stale"))?;
                let declarations = checked.data_members(record);
                if *symbol != data_symbol
                    || literal.type_symbol != data_symbol
                    || literal.case_symbol.is_some()
                    || authored.len() != retained.len()
                    || declarations.len() != retained.len()
                {
                    return unsupported("record construction changed its exact field roster");
                }
                let mut seen = Vec::new();
                for (ordinal, (source, field)) in authored.iter().zip(retained).enumerate() {
                    let declaration = declarations
                        .iter()
                        .find_map(|member| match member {
                            checked_trees::data::DataMember::Field(declaration)
                                if declaration.symbol == field.field
                                    && !declaration.relevance.is_erased() =>
                            {
                                Some(declaration)
                            }
                            _ => None,
                        })
                        .ok_or(LoweringError::Unsupported(
                            "record field is not an exact scalar declaration",
                        ))?;
                    if source.field_symbol != field.field || seen.contains(&field.field) {
                        return unsupported("record fields were duplicated or reordered");
                    }
                    seen.push(field.field);
                    let role = CheckedScalarExpressionRole::RecordField {
                        expression,
                        field_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("record field ordinal overflow")
                        })?,
                    };
                    operand_roles.push(role);
                    let primitive = validate_operand(
                        checked,
                        machine,
                        state,
                        result.statement_index,
                        role,
                        field.value,
                        source.value,
                    )?;
                    if checked.primitive_type_reference(declaration.type_reference)
                        != Some(primitive)
                    {
                        return unsupported("record field changed its scalar carrier");
                    }
                }
            }
            CheckedStructuralValueKind::Case(construction) => {
                if construction.expression != expression
                    || checked.normalized_type_identity(construction.type_reference)
                        != checked.normalized_type_identity(reference)
                {
                    return unsupported(
                        "structural construction substituted its exact fresh case owner",
                    );
                }
                let fields = crate::scalar_computations::cases::source::construction(
                    checked,
                    &construction,
                )?;
                for (ordinal, (expression, handle)) in fields.iter().enumerate() {
                    let role = CheckedScalarExpressionRole::StructuralValueField {
                        expression: construction.expression,
                        field_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("case field ordinal overflow")
                        })?,
                    };
                    operand_roles.push(role);
                    validate_operand(
                        checked,
                        machine,
                        state,
                        result.statement_index,
                        role,
                        *handle,
                        *expression,
                    )?;
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
                        | CheckedScalarExpressionRole::RecordField { .. }
                        | CheckedScalarExpressionRole::StructuralValuePattern { .. }
                        | CheckedScalarExpressionRole::StructuralValueField { .. }
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
        if let CheckedScalarExpressionRole::StructuralValueField {
            expression: constructor,
            field_ordinal,
        } = role
            && constructor == expression
        {
            let source = validation::scalar_case_constructor(&checked.typed, expression).ok_or(
                LoweringError::Unsupported("structural field lost its authored constructor"),
            )?;
            return source
                .fields
                .get(field_ordinal as usize)
                .map(|(_, expression, primitive)| (*expression, *primitive))
                .ok_or(LoweringError::Unsupported(
                    "structural field ordinal escaped its constructor",
                ));
        }
        if let CheckedScalarExpressionRole::RecordField {
            expression: owner,
            field_ordinal,
        } = role
            && owner == expression
            && let ExpressionNode::StructLiteral(literal) =
                checked.expression_table.expression(expression)
        {
            let field = checked
                .expression_table
                .struct_fields(literal.fields)
                .get(field_ordinal as usize)
                .ok_or(LoweringError::Unsupported("record operand ordinal missing"))?;
            let declaration = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == literal.type_symbol)
                .and_then(|data| {
                    checked
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            checked_trees::data::DataMember::Field(declaration)
                                if declaration.symbol == field.field_symbol
                                    && !declaration.relevance.is_erased() =>
                            {
                                Some(declaration)
                            }
                            _ => None,
                        })
                })
                .ok_or(LoweringError::Unsupported(
                    "record operand declaration missing",
                ))?;
            return checked
                .primitive_type_reference(declaration.type_reference)
                .map(|primitive| (field.value, primitive))
                .ok_or(LoweringError::Unsupported(
                    "record operand has no scalar carrier",
                ));
        }
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

/// Whole selected locals retain their statement establishment provenance through
/// later observation, transfer, or disposal. The operation/edge receivers check
/// the selected transfer schedule independently.
fn validate_local_ownership(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    symbol: SymbolHandle,
    multiplicity: language_semantics::Multiplicity,
    discard_result_on_return: bool,
) -> Result<(), LoweringError> {
    use language_semantics::{Multiplicity, PermissionEventKind, PermissionEventSource};
    let (_, source) = crate::scalar_source_custody::authored_state(checked, state)?;
    if !symbol.is_valid()
        || checked.state_parameters(source).iter().any(|parameter| parameter.symbol == symbol)
        || checked.statement_table.statements(source.statement_nodes).iter().filter(|statement| matches!(statement, StatementNode::LocalData(local) if local.symbol == symbol)).count() != 1 {
        return unsupported("structural local has ambiguous source identity");
    }
    let establishment_source = PermissionEventSource::Statement {
        statement_index: statement as usize,
    };
    let provenance = language_semantics::PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: establishment_source,
    };
    let ownership = &checked.facts.flow.ownership;
    let mut establishments = 0;
    let mut drops = 0;
    let mut transfers = 0;
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine
            && event.state_symbol == state
            && event.root == facts::PlaceRoot::Symbol(symbol)
    }) {
        if event.access != language_semantics::PermissionAccess::Owned
            || event.multiplicity != multiplicity
            || multiplicity != Multiplicity::Affine
            || event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || event.provenance != provenance
            || event.obligation_live
            || ownership
                .segments
                .span(event.segments)
                .is_none_or(|segments| !segments.is_empty())
        {
            return unsupported("structural local changed its whole no-code ownership provenance");
        }
        match event.kind {
            PermissionEventKind::Establish if event.source == establishment_source => {
                establishments += 1
            }
            PermissionEventKind::AffineDrop if event.source == PermissionEventSource::StateExit => {
                drops += 1
            }
            PermissionEventKind::Transfer => transfers += 1,
            _ => return unsupported("structural local changed its ownership event"),
        }
    }
    if establishments != usize::from(multiplicity == Multiplicity::Affine)
        || drops > 1
        || (multiplicity == Multiplicity::Affine && drops == 0 && transfers == 0)
        || (multiplicity != Multiplicity::Unrestricted
            && discard_result_on_return
            && (drops != 1 || transfers != 0))
    {
        return unsupported("structural local lost its exact establishment or final disposition");
    }
    Ok(())
}
