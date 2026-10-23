//! Independently checked integer rules: closed integer denotation, open-term
//! ring normalization, the fixed native integer readings, and the affine,
//! cast, forbidden-root and shift normalizations a proof node may cite.

pub(crate) mod closed_integer;
pub(crate) mod fixed_integer;
pub(crate) mod integer_affine;
pub(crate) mod integer_cast;
pub(crate) mod integer_forbidden_root;
pub(crate) mod integer_shift;
pub(crate) mod open_terms;
pub(crate) mod wrapping_equality;

/// Test-only scalar-term shortcut shared by the integer rule suites.
#[cfg(test)]
pub(crate) fn value(
    id: u64,
    integer_type: semantic_vocabulary::IntegerType,
) -> semantic_vocabulary::ScalarTerm {
    semantic_vocabulary::ScalarTerm::value(
        semantic_vocabulary::ValueId::new(id).expect("value"),
        semantic_vocabulary::ScalarType::Integer(integer_type),
    )
}
