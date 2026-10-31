//! Optimizer module role: executable entrance. Access text to object-format data.
//! The caller retains source admission; these functions grant no publication authority.
use super::RelocationFreeObjectError;
mod production;
mod validation;
pub(crate) use production::construct_relocation_free_object_from_text;
pub(crate) use validation::validate_relocation_free_object_from_text;
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RelocationFreeObjectFromTextError {
    InvalidObject(RelocationFreeObjectError),
    LengthOverflow,
    MissingSemanticEntry,
    SourceMismatch,
}
