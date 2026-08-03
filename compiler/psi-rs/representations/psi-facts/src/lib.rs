#![forbid(unsafe_code)]

//! Durable target-neutral fact vocabulary produced by Psi checking.

mod definitions;
mod model;
mod place_resolution;
mod plan;
mod view;
mod write_frame;

pub use definitions::*;
pub use model::*;
pub use plan::*;
pub use view::*;
pub use write_frame::*;

pub use place_resolution::payload_variant_for_field;
pub(crate) use place_resolution::{
    canonical_place_label, effective_member_symbol, resolve_place_member_symbol,
};

#[cfg(test)]
mod tests;
