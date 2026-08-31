#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Canonical target-legal operation custody for the production Omega realization pipeline.
//!
//! The public model is split by scalar and structural carriers. Representation-
//! owned validation and canonical identity encoding descend through independent
//! leaves; this entrance only exposes the stable data and identity surface.

mod identity;
mod model;
mod validation;

pub use identity::{
    legalized_operation_plan_identity, legalized_operation_plan_identity_v9_legacy,
};
pub use model::*;

#[cfg(test)]
mod tests;
