//! Exact integer-literal carrier remapping for cast-bound production.

use semantic_vocabulary::ScalarTerm;

pub(in super::super) fn remap_integer_literal(
    literal: &ScalarTerm,
    target_type: semantic_vocabulary::IntegerType,
) -> Option<ScalarTerm> {
    let (source_type, value) = literal.integer_value()?;
    let value = source_type.exact_cast_value_to(target_type, value)?;
    ScalarTerm::integer(target_type, value).ok()
}
