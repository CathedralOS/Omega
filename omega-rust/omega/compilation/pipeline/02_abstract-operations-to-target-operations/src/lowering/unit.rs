//! Operation-level call, storage, and scalar lowering shared by control graphs.
pub(super) mod argument_roots;
pub(super) mod boundary_call;
pub(super) mod known_integers;
mod owned_store_home;
mod projected_result;
pub(super) mod roster_arguments;
pub(super) mod scalar_definitions;
mod structural_scalar;
pub(super) use structural_scalar::lower_field_store;
pub(super) mod write_only_primitive_store;
