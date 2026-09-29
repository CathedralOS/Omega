//! Replay custody for a `MutableSliceView` selection value carrying a
//! `MutableBorrow` element-view argument. The checked plan recorded the
//! authored `place.as_mut_slice()` view's exact collection root symbol and
//! member path; replay rebuilds that collection place and requires mutable
//! access on the argument, the exact path, and the projected leaf's identity
//! equal to the argument's recorded collection identity. Like `shared_borrow`,
//! a mutable view join is provenance pass-through: it leaves owned custody
//! records untouched — loan exclusivity is the checked planner's
//! `Mutable`-kind roster, not a replay concern.

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
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
        return unsupported("mutable slice view lost its authored view call");
    };
    if !call.arguments.is_empty() {
        return unsupported("mutable slice view call gained arguments");
    }
    if argument.access != checked_trees::CheckedStructuralAccess::MutableBorrow {
        return unsupported("mutable slice view changed its planned access");
    }
    let (root, checked_path) = super::borrowed_slice_view::authored_collection_path(
        checked,
        machine,
        authored,
        call.receiver,
    )?;
    if checked_path != argument.path {
        return unsupported("mutable slice view path does not match its projected path");
    }
    let leaf = validation::expression_result_type_reference(
        &checked.typed,
        machine,
        authored,
        call.receiver,
    )
    .ok_or(LoweringError::Unsupported(
        "mutable slice view lost its collection type",
    ))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity {
        return unsupported("mutable slice view does not project its collection");
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        return unsupported("mutable slice view receiver is not an exact place");
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("mutable slice view root is not an exact name");
    }
    let is_self_root = matches!(
        checked.expression_table.name_path_members(name.members),
        [spelling] if spelling.is_self_receiver()
    );
    if !super::owned_selection::source_plan_matches(
        checked,
        authored.symbol,
        name.symbol,
        &argument.source,
    ) {
        return unsupported("mutable slice view source does not match its root");
    }
    if is_self_root {
        // `self` names the machine's attached storage: the checked plan carries
        // it as `StructuralLocal{machine_symbol}` and the receiver parameter
        // owns no authored local slot.
        if !checked
            .state_parameters(authored)
            .iter()
            .any(|parameter| parameter.is_self)
        {
            return unsupported("mutable slice view self root has no self parameter");
        }
    } else if !checked
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
            return unsupported("mutable slice view loses its authored root");
        };
        if locals.next().is_some() || index >= statement as usize || !local.initial_value.is_valid()
        {
            return unsupported("mutable slice view root is not uniquely established");
        }
    }
    // The lent place is the projected collection, not the root's whole record:
    // an attached receiver may legitimately hold members (services, loans) the
    // pipeline cannot carry while the projected array keeps plain storage.
    if !validation::has_linear_owned_contents(
        &checked.typed,
        validation::unwrapped_type_reference(&checked.typed, leaf).unwrap_or(leaf),
    ) {
        return unsupported(
            "mutable slice view root is not a structural place the pipeline can carry",
        );
    }
    Ok(())
}
