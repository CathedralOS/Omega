//! Independently checked integer rules: closed integer denotation, open-term
//! ring normalization, and the affine, cast, forbidden-root and shift
//! normalizations a proof node may cite.

pub(crate) mod closed_integer;
pub(crate) mod integer_affine;
pub(crate) mod integer_cast;
pub(crate) mod integer_forbidden_root;
pub(crate) mod integer_shift;
pub(crate) mod open_terms;
