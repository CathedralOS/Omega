//! A declared view binding's type read from its authored initializer.
//! `byte_literals` replays constructor byte literals against their declared
//! carrier.

use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionHandle;
use checked_trees::types::TypeReferenceHandle;

pub(crate) mod byte_literals;

/// The type a view-binding `let` declares. Both authored spellings bind the
/// same slice: `let v: &[u8] = coll[a..b]` names the view directly, while
/// `let v: u8 = coll[a..b]` names only its element — the binding's declared
/// type is then the expression's own slice-view result, exactly as the
/// checker resolved it. A non-view declaration is returned unchanged.
pub(crate) fn declared_view_binding_type(
    checked: &CheckedTrees,
    declared: TypeReferenceHandle,
    initial_value: ExpressionHandle,
) -> TypeReferenceHandle {
    if validation::reference_shell(&checked.typed, declared).is_some() {
        return declared;
    }
    let result = checked.facts.resolved_types.result(initial_value);
    if result.is_valid() && validation::slice_view_shell(&checked.typed, result, &[]).is_some() {
        result
    } else {
        declared
    }
}
