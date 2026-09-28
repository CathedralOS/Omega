//! Optimizer module role: executable entrance. Placed text to object-format data.
//! The caller retains source admission; these functions grant no publication authority.
use super::RelocationFreeObjectError;
mod production;
mod validation;
pub use production::construct_relocation_free_object_from_text;
pub use validation::validate_relocation_free_object_from_text;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectFromTextError {
    InvalidObject(RelocationFreeObjectError),
    LengthOverflow,
    MissingSemanticEntry,
    SourceMismatch,
}
