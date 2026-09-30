//! Builtin scalar carrier spellings, recognized before any symbol exists.
//!
//! The integer carriers are exactly the `LandedIntegerType` names. With `bool`
//! they form the structural scalars: carriers whose values have a canonical
//! const-index identity. The float formats complete the builtin scalars; their
//! values land at a declared format but carry no structural identity.

use numerics::literals::LandedIntegerType;

/// `bool` or an integer carrier.
pub(crate) fn is_structural_scalar_carrier(name: &str) -> bool {
    name == "bool" || LandedIntegerType::from_name(name).is_some()
}

/// A structural scalar or a float format.
pub(crate) fn is_builtin_scalar_carrier(name: &str) -> bool {
    is_structural_scalar_carrier(name) || matches!(name, "f32" | "f64")
}
