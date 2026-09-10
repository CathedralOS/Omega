//! Rejoin whole owned actuals before assigning Terminal places.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::state::State;
use checked_trees::statement::StatementNode;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    BorrowCallFact, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::Multiplicity;

use super::borrow_rows;
use crate::{LoweringError, unsupported};

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    target_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &BorrowCallFact,
    access_position: usize,
) -> Result<usize, LoweringError> {
    if let CheckedUnitStructuralArgumentSourcePlan::ArrayLocal { symbol } = argument.source {
        return validate_array_local(
            checked,
            state,
            target_type,
            expression,
            argument,
            call,
            access_position,
            symbol,
        );
    }
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = argument.source
    else {
        return unsupported("computed owned argument requires a whole source parameter");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("computed owned argument requires an authored parameter name");
    };
    let parameters = checked.state_parameters(state);
    let source = parameters
        .iter()
        .filter(|parameter| {
            checked
                .primitive_type_reference(parameter.type_reference)
                .is_none()
        })
        .nth(parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "computed owned argument has no source parameter",
        ))?;
    if !checked.expression_table.expression_is_valid(expression)
        || !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || name.members.count() != 1
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || source.symbol != name.symbol
        || parameters
            .iter()
            .filter(|parameter| parameter.symbol == source.symbol)
            .count()
            != 1
        || source.is_self
        || source.is_const
        || source.is_mutable
        || !argument.path.is_empty()
        || argument.access != CheckedStructuralAccess::Owned
        || [source.type_reference, target_type]
            .iter()
            .any(|reference| {
                !checked
                    .type_reference_table
                    .contains_type_reference(*reference)
                    || (!matches!(
                        checked.type_reference_table.type_reference(*reference),
                        TypeReferenceNode::Named { .. }
                    ) && !validation::is_closed_primitive_array_type(checked, *reference))
                    || checked.primitive_type_reference(*reference).is_some()
                    || !matches!(
                        checked.type_multiplicity(*reference),
                        Multiplicity::Unrestricted | Multiplicity::Affine
                    )
            })
        || checked.normalized_type_identity(source.type_reference)
            != checked.normalized_type_identity(target_type)
        || checked.normalized_type_identity(target_type).into_string() != argument.type_identity
    {
        return unsupported("computed owned argument substituted its source, type, or access");
    }
    // Borrow observations record the authored read. Affine transfer has its
    // own permission event; this read does not replace that disposition.
    borrow_rows::validate_shared_argument_at(checked, call, access_position, source.symbol)?;
    if checked.type_multiplicity(source.type_reference) == Multiplicity::Affine {
        let (owner, _) = super::authored_state(checked, state.symbol)?;
        let ownership = &checked.facts.flow.ownership;
        let mut transfers = ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == owner.symbol
                    && event.state_symbol == state.symbol
                    && event.root == facts::PlaceRoot::Symbol(source.symbol)
                    && event.source
                        == language_semantics::PermissionEventSource::Call {
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                            target_symbol: call.target_symbol,
                        }
            });
        let transfer = transfers.next().ok_or(LoweringError::Unsupported(
            "computed owned argument lost its affine transfer permission",
        ))?;
        if transfers.next().is_some()
            || transfer.kind != language_semantics::PermissionEventKind::Transfer
            || transfer.access != language_semantics::PermissionAccess::Owned
            || transfer.multiplicity != Multiplicity::Affine
            || transfer.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || transfer.provenance != language_semantics::PermissionProvenance::Unknown
            || transfer.obligation_live
            || ownership
                .segments
                .span(transfer.segments)
                .is_none_or(|segments| !segments.is_empty())
        {
            return unsupported("computed owned argument changed its affine transfer permission");
        }
    }
    Ok(access_position)
}

fn validate_array_local(
    checked: &CheckedTrees,
    state: &State,
    target_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &BorrowCallFact,
    access_position: usize,
    symbol: symbols::SymbolHandle,
) -> Result<usize, LoweringError> {
    let table = &checked.expression_table;
    if !table.expression_is_valid(expression) {
        return unsupported("computed owned array local has a stale authored expression");
    }
    let ExpressionNode::Name(name) = table.expression(expression) else {
        return unsupported("computed owned array local requires an authored whole name");
    };
    if !symbol.is_valid()
        || name.symbol != symbol
        || name.head_symbol != symbol
        || name.members.count() != 1
        || table.name_path_members(name.members).len() != 1
        || !argument.path.is_empty()
        || argument.access != CheckedStructuralAccess::Owned
        || checked
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == symbol)
    {
        return unsupported("computed owned array local substituted its source or access");
    }
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
            _ => None,
        });
    let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(
        "computed owned array argument has no authored local",
    ))?;
    if locals.next().is_some()
        || ordinal >= call.statement_index
        || local.is_mutable
        || !table.expression_is_valid(local.initial_value)
        || [local.type_reference, target_type].iter().any(|reference| {
            !checked
                .type_reference_table
                .contains_type_reference(*reference)
                || !validation::is_closed_primitive_array_type(checked, *reference)
                || checked.type_multiplicity(*reference) != Multiplicity::Unrestricted
        })
        || checked.normalized_type_identity(local.type_reference)
            != checked.normalized_type_identity(target_type)
        || checked.normalized_type_identity(target_type).into_string() != argument.type_identity
    {
        return unsupported("computed owned array local changed its declaration, order, or type");
    }
    borrow_rows::validate_shared_argument_at(checked, call, access_position, symbol)?;
    Ok(access_position)
}
