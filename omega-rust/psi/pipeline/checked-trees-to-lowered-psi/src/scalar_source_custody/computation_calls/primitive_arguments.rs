//! Source custody of whole primitive borrows, independent of lowered places.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::state::State;
use checked_trees::statement::StatementNode;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    BorrowAccessKind, BorrowCallFact, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

use super::borrow_rows;
use crate::{LoweringError, unsupported};

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    target_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    borrow_call: &BorrowCallFact,
    access_position: Option<usize>,
) -> Result<usize, LoweringError> {
    let (target_referent, target_access) = primitive_reference(checked, target_type)?;
    if !argument.path.is_empty() || argument.access != target_access {
        return unsupported("computed primitive argument substituted its path or formal access");
    }
    let table = &checked.expression_table;
    if !table.expression_is_valid(expression) {
        return unsupported("computed primitive argument has a stale authored expression");
    }
    let (named, explicit_access) = match table.expression(expression) {
        ExpressionNode::Borrow(borrow) => (borrow.target, Some(access(borrow.access))),
        ExpressionNode::Name(_) => (expression, None),
        _ => return unsupported("computed primitive argument requires a whole named referent"),
    };
    if !table.expression_is_valid(named) {
        return unsupported("computed primitive argument has a stale borrowed expression");
    }
    let ExpressionNode::Name(name) = table.expression(named) else {
        return unsupported("computed primitive argument requires an unprojected named referent");
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || table.name_path_members(name.members).len() != 1
        || name.members.count() != 1
        || explicit_access.is_some_and(|access| access != target_access)
    {
        return unsupported("computed primitive argument substituted its authored borrow");
    }
    let parameters = checked.state_parameters(state);
    let source_referent = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let source = parameters
                .iter()
                .filter(|parameter| {
                    checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
                })
                .nth(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "computed primitive argument has no source structural parameter",
                ))?;
            if source.is_self || source.is_const || source.symbol != name.symbol
                || parameters.iter().filter(|parameter| parameter.symbol == name.symbol).count() != 1
                || checked.statement_table.statements(state.statement_nodes).iter().any(|statement| {
                    matches!(statement, StatementNode::LocalData(local) if local.symbol == name.symbol)
                })
            {
                return unsupported("computed primitive argument substituted its source parameter");
            }
            let (referent, declared_access) = primitive_reference(checked, source.type_reference)?;
            // Resource access comes from the exact source declaration and
            // authored borrow. A target's requested access cannot widen it.
            if borrow_kind(declared_access)?
                .direct_reborrow_effect(&borrow_kind(target_access)?)
                .is_none()
            {
                return unsupported(
                    "computed primitive argument widens its declared source access",
                );
            }
            if explicit_access.is_none() && target_access != CheckedStructuralAccess::SharedBorrow {
                if declared_access != CheckedStructuralAccess::MutableBorrow
                    || target_access != CheckedStructuralAccess::MutableBorrow
                {
                    return unsupported(
                        "computed primitive argument lacks an authored exclusive borrow",
                    );
                }
                validate_direct_forwarding(checked, state, statement, name.symbol)?;
            }
            referent
        }
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } => {
            if symbol != name.symbol
                || explicit_access.is_none()
                || parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("computed primitive argument substituted its local borrow");
            }
            let mut locals = checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(|(ordinal, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some((ordinal, local))
                    }
                    _ => None,
                });
            let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(
                "computed primitive argument has no authored local",
            ))?;
            if locals.next().is_some()
                || ordinal >= statement as usize
                || !local.is_mutable
                || !table.expression_is_valid(local.initial_value)
            {
                return unsupported(
                    "computed primitive argument has no preceding initialized mutable local",
                );
            }
            plain_primitive(checked, local.type_reference)?;
            local.type_reference
        }
        _ => return unsupported("computed primitive argument has unsupported source custody"),
    };
    let source_identity = checked.typed.normalized_type_identity(source_referent);
    if checked.primitive_type_reference(source_referent)
        != checked.primitive_type_reference(target_referent)
        || source_identity != checked.typed.normalized_type_identity(target_referent)
        || source_identity.into_string() != argument.type_identity
    {
        return unsupported("computed primitive argument substituted its normalized referent type");
    }
    // Bare forwarding observes the reference carrier. An explicit borrow's
    // access row records the borrowed referent's exact authored access.
    let expected_kind = match explicit_access {
        Some(access) => borrow_kind(access)?,
        None => BorrowAccessKind::Read,
    };
    if argument.access == CheckedStructuralAccess::SharedBorrow
        && let Some(position) = access_position
    {
        borrow_rows::validate_shared_argument_at(checked, borrow_call, position, name.symbol)?;
        Ok(position)
    } else {
        borrow_rows::validate_argument(checked, borrow_call, name.symbol, expected_kind)
    }
}

fn primitive_reference(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<(TypeReferenceHandle, CheckedStructuralAccess), LoweringError> {
    if !checked
        .type_reference_table
        .contains_type_reference(reference)
    {
        return unsupported("computed primitive argument has a stale reference type");
    }
    let TypeReferenceNode::Reference {
        referee,
        access: declared_access,
        ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("computed structural argument requires a primitive reference");
    };
    plain_primitive(checked, *referee)?;
    Ok((*referee, access(*declared_access)))
}

fn plain_primitive(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<(), LoweringError> {
    if !checked
        .type_reference_table
        .contains_type_reference(reference)
        || !matches!(
            checked.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        )
        || checked.primitive_type_reference(reference).is_none()
    {
        return unsupported(
            "computed structural argument requires an exact plain primitive referent",
        );
    }
    Ok(())
}

fn access(access: ReferenceAccess) -> CheckedStructuralAccess {
    match access {
        ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    }
}

fn borrow_kind(access: CheckedStructuralAccess) -> Result<BorrowAccessKind, LoweringError> {
    Ok(match access {
        CheckedStructuralAccess::SharedBorrow => BorrowAccessKind::Read,
        CheckedStructuralAccess::MutableBorrow => BorrowAccessKind::Mutable,
        CheckedStructuralAccess::WriteOnlyBorrow => BorrowAccessKind::WriteOnly,
        CheckedStructuralAccess::Owned => {
            return unsupported("computed primitive argument requires borrowed access");
        }
    })
}

fn validate_direct_forwarding(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    symbol: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    // This slice does not carry restoration certificates for a descendant
    // loan. A bare carrier cannot stand in for that missing resource custody.
    for (_, loan) in checked.facts.borrow.loans.iter() {
        if loan.statement_index <= statement as usize
            && loan.last_use_statement_index >= statement as usize
            && (loan.root_symbol == symbol
                || loan.owner_symbol == symbol
                || loan.source_owner_symbol == symbol)
        {
            return unsupported(
                "computed primitive forwarding requires unsupported loan restoration",
            );
        }
    }
    let prefix = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement as usize)
        .ok_or(LoweringError::Unsupported(
            "computed primitive forwarding has no authored prefix",
        ))?;
    for statement in prefix {
        if let StatementNode::Assignment(assignment) = statement
            && let ExpressionNode::Name(name) =
                checked.expression_table.expression(assignment.target)
            && (name.symbol == symbol || name.head_symbol == symbol)
        {
            return unsupported("computed primitive forwarding requires unchanged entry custody");
        }
    }
    Ok(())
}
