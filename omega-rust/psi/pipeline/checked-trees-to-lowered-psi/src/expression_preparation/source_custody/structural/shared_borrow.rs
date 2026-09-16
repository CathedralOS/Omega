//! Replay custody for a `Reference` selection value carrying a `SharedBorrow`
//! argument. The checked plan recorded the authored `&place` expression's
//! exact root symbol and member path; replay rebuilds that place and requires
//! shared access on the authored borrow, the result type and the argument, the
//! exact member path, the projected leaf's identity equal to the result's
//! record referent, and an established plain-structural root place. There is
//! no owned receipt or transfer to consume: a shared-borrow join is provenance
//! pass-through, so unlike `validate_projection` this leaf leaves owned
//! custody records untouched.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, unsupported,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    expression: ExpressionHandle,
    reference: checked_trees::types::TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression) else {
        return unsupported("borrowed selection value lost its authored borrow");
    };
    if borrow.access != language_semantics::ReferenceAccess::Shared {
        return unsupported("borrowed selection value changed its authored access");
    }
    let Some(referent) = super::shared_borrow_record_referent(checked, reference) else {
        return unsupported("borrowed selection result lost its record referent");
    };
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("borrowed selection changed its planned access");
    }
    let mut checked_path = Vec::new();
    let mut cursor = borrow.target;
    let root = loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("borrowed selection target uses a case member");
                }
                let receiver = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(authored),
                    member.receiver,
                )
                .and_then(|receiver| validation::unwrapped_type_reference(&checked.typed, receiver))
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection lost its receiver type",
                ))?;
                let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(receiver)
                else {
                    return unsupported("borrowed selection receiver is not a named record");
                };
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|owner| owner.symbol == *symbol)
                    .ok_or(LoweringError::Unsupported(
                        "borrowed selection field owner is absent",
                    ))?;
                let field = validation::exact_data_member_field(
                    &checked.typed,
                    owner,
                    member.member_symbol,
                    member.member.as_str(),
                    None,
                )
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection lost its member declaration",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("borrowed selection reads an erased member");
                }
                checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                cursor = member.receiver;
            }
            ExpressionNode::Name(_) => break cursor,
            _ => return unsupported("borrowed selection target is not an exact place"),
        }
    };
    checked_path.reverse();
    if checked_path != argument.path {
        return unsupported("borrowed selection path does not match its projected path");
    }
    let leaf = validation::expression_result_type_reference(
        &checked.typed,
        machine,
        authored,
        borrow.target,
    )
    .ok_or(LoweringError::Unsupported(
        "borrowed selection lost its target type",
    ))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity
        || checked.normalized_type_identity(leaf) != checked.normalized_type_identity(referent)
    {
        return unsupported("borrowed selection does not project the result referent");
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        return unsupported("borrowed selection target is not an exact place");
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("borrowed selection root is not an exact name");
    }
    if !super::owned_selection::source_plan_matches(
        checked,
        authored.symbol,
        name.symbol,
        &argument.source,
    ) {
        return unsupported("borrowed selection source does not match its root");
    }
    if !checked
        .state_parameters(authored)
        .iter()
        .any(|parameter| parameter.symbol == name.symbol)
    {
        let mut locals = checked
            .statement_table
            .statements(authored.statement_nodes)
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| match candidate {
                StatementNode::LocalData(local) if local.symbol == name.symbol => {
                    Some((index, local))
                }
                _ => None,
            });
        let Some((index, local)) = locals.next() else {
            return unsupported("borrowed selection loses its authored root");
        };
        if locals.next().is_some() || index >= statement as usize || !local.initial_value.is_valid()
        {
            return unsupported("borrowed selection root is not uniquely established");
        }
    }
    let root_reference =
        validation::expression_result_type_reference(&checked.typed, machine, authored, root)
            .ok_or(LoweringError::Unsupported(
                "borrowed selection lost its root type",
            ))?;
    if !validation::has_plain_owned_contents_with_numeric_constraints(
        &checked.typed,
        validation::unwrapped_type_reference(&checked.typed, root_reference)
            .unwrap_or(root_reference),
    ) {
        return unsupported("borrowed selection root is not a plain structural place");
    }
    Ok(())
}
