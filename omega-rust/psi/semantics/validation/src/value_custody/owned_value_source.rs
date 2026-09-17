//! Whole plain-owned occurrences share source identity across records and sums.
//! This classifies the named storage only; flow checking must establish its
//! availability and each selected transfer's residual ownership independently.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

/// Resolve a whole local or parameter without erasing a borrow, qualification,
/// or nominal type. A projection is not its containing owner's whole value.
pub fn plain_owned_value_source(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    whole_owned_value_source(program, expression, reference)
        .filter(|_| crate::has_plain_owned_contents(program, reference))
}

/// The same whole-place resolution for a linear carrier: the named storage is
/// the whole value, and the caller's claim accounting (not this predicate)
/// decides whether the move is available on every path that needs it.
pub fn linear_owned_value_source(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    whole_owned_value_source(program, expression, reference).filter(|_| {
        program.type_multiplicity(reference) == language_semantics::Multiplicity::Linear
            && crate::has_linear_owned_contents(program, reference)
    })
}

/// A whole local or parameter name whose declared type is exactly `reference`.
/// Contents and multiplicity remain the caller's question: a shared borrow,
/// a linear claim, and a plain affine value all resolve to the same place.
pub(crate) fn whole_owned_value_source(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if path.symbol != path.head_symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || !matches!(
            program.symbols.get(path.symbol).kind,
            SymbolKind::Local | SymbolKind::Parameter
        )
    {
        return None;
    }
    let actual = crate::value_custody::expression_types::named_value_type_reference(program, path)?;
    (program.normalized_type_identity(actual) == program.normalized_type_identity(reference))
        .then_some(path.symbol)
}
