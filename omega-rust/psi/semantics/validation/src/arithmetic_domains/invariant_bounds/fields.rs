//! Direct owned projections contribute storage bounds under their exact owner.

use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn type_reference(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Name(receiver) = program.expression_table.expression(member.receiver)
    else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(receiver.members) else {
        return None;
    };
    let parameter = program.state_parameters(state).iter().find(|parameter| {
        receiver.symbol.is_valid()
            && receiver.symbol == receiver.head_symbol
            && parameter.symbol == receiver.symbol
            && parameter.name == *name
            && !parameter.is_self
            && !parameter.is_mutable
            && !parameter.is_const
    })?;
    // Reference and generic receivers need load or instantiated-field evidence.
    // A named record contributes only its stored field's own enforced bounds.
    let TypeReferenceNode::Named { symbol: owner, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let declaration = program
        .data_definitions()
        .iter()
        .find(|declaration| owner.is_valid() && declaration.symbol == *owner)?;
    let selected = program.symbols.get(member.member_symbol);
    if !member.member_symbol.is_valid()
        || selected.kind != SymbolKind::Field
        || selected.parent != *owner
        || member.case_variant.is_some()
    {
        return None;
    }
    crate::places::exact_data_member_field(
        program,
        declaration,
        member.member_symbol,
        member.member.as_str(),
        None,
    )
    .map(|field| field.type_reference)
}
