//! Scalar types: `bounded_integer_type` restricts a fixed integer carrier to
//! one interval, `qualified_scalar_type` pairs a scalar type with its
//! qualification set, `scalar_refinement` binds a proposition to one isolated
//! scalar formal, and `ieee_float_comparison_operation` names the executable
//! float relations.

pub(crate) mod bounded_integer_type;
pub(crate) mod ieee_float_comparison_operation;
pub(crate) mod qualified_scalar_type;
pub(crate) mod scalar_refinement;
