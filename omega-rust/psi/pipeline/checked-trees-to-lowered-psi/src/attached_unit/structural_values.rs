//! Fresh construction and selected value joins share the ordinary result
//! namespace. Case and record identities remain structural; field and dispatch
//! operands share scalar evaluation. Each selected owner transfers to one continuation place,
//! which later statements consume just like a completed call result.

use super::*;

mod emission;
mod locals;
mod record;
pub(crate) mod source_custody;

pub(crate) use emission::emit;
pub(super) use locals::bind_local;

/// Closed record storage has a nominal declaration and no variant selection.
/// Qualification and ownership shells remain checked by the existing validator.
pub(super) fn plain_record(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> bool {
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    validation::has_plain_owned_contents_with_numeric_constraints(&checked.typed, reference)
        && checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)
            .is_some_and(|data| {
                !checked
                    .data_members(data)
                    .iter()
                    .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
            })
}
