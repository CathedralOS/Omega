//! Storage and the places in it: `authored_storage` is closed source-authored
//! geometry, `target_carrier_layout` the target-supplied carrier geometry,
//! `structural_places` the machine-place roots and affine construction
//! coordinates, `content` the content projections over those places, and
//! `reference_origins` a returned borrow's extent over supplied storage.

pub(crate) mod atomic_cell;
pub(crate) mod authored_storage;
pub(crate) mod content;
pub(crate) mod reference_origins;
pub(crate) mod structural_places;
pub(crate) mod target_carrier_layout;
