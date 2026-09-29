//! Rejoin whole owned actuals before assigning Terminal places.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::state::State;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    BorrowCallFact, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::Multiplicity;

use super::borrow_rows;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    target_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &BorrowCallFact,
    access_position: usize,
) -> Result<usize, LoweringError> {
    if let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = argument.source {
        return validate_copy_local(
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
    let source = super::structural_parameter_at(
        checked,
        parameters,
        parameter_index,
        "computed owned argument has no source parameter",
    )?;
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

// Whole local copies rejoin their declaration and authored observation. Closed
// plain-owned contents exclude loans and cleanup; affine moves require a
// separate transfer proof and cannot enter through this copy route.
fn validate_copy_local(
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
        return unsupported("computed owned copy local has a stale authored expression");
    }
    let ExpressionNode::Name(name) = table.expression(expression) else {
        return unsupported("computed owned copy local requires an authored whole name");
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
        return unsupported("computed owned copy local substituted its source or access");
    }
    let local = super::preceding_local(
        checked,
        state,
        symbol,
        call.statement_index,
        "computed owned copy argument has no authored local",
        "computed owned copy local changed its declaration, order, or type",
    )?;
    if local.is_mutable
        || [local.type_reference, target_type].iter().any(|reference| {
            !checked
                .type_reference_table
                .contains_type_reference(*reference)
                || checked.primitive_type_reference(*reference).is_some()
                || !validation::has_plain_owned_contents(checked, *reference)
                || checked.type_multiplicity(*reference) != Multiplicity::Unrestricted
        })
        || checked.normalized_type_identity(local.type_reference)
            != checked.normalized_type_identity(target_type)
        || checked.normalized_type_identity(target_type).into_string() != argument.type_identity
    {
        return unsupported("computed owned copy local changed its declaration, order, or type");
    }
    borrow_rows::validate_shared_argument_at(checked, call, access_position, symbol)?;
    Ok(access_position)
}
