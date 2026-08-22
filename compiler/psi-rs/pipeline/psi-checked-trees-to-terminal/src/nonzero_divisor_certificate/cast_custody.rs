//! Side-local custody facade for an exact integer-cast chain.

use psi_core::ScalarTerm;

mod chain;
mod completion;

pub(super) use completion::prove_from_root;

pub(super) fn remap_integer_literal(
    literal: &ScalarTerm,
    target_type: psi_core::IntegerType,
) -> Option<ScalarTerm> {
    let (source_type, value) = literal.integer_value()?;
    let value = source_type.exact_cast_value_to(target_type, value)?;
    ScalarTerm::integer(target_type, value).ok()
}
