//! Structural values: claims, declarations, effects, stores and restoration
//! windows, static paths, owned traversal and partial-affine residuals, zero
//! initialization, primitive places, record fields, inline carriers, shared
//! mutation and nominal cleanup.

pub(crate) mod atomic_cell;
pub(crate) mod inline_carriers;
pub mod nominal_cleanup;
pub mod owned_structure;
pub(crate) mod partial_affine;
pub(crate) mod primitive_place;
pub(crate) mod record_field;
pub(crate) mod shared_mutation;
pub(crate) mod static_path;
pub(crate) mod structural_claims;
pub(crate) mod structural_declarations;
pub(crate) mod structural_effect;
pub(crate) mod structural_store;
pub(crate) mod structural_window;
pub(crate) mod zeroed_structural;
