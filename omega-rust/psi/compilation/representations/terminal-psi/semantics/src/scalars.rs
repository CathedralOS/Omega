//! Scalars: goal-free leaf schemas and semantics, proof-bearing scalars,
//! scalar ordering and observations, representation views, field requirements,
//! scalar arrays and identity-preserving flow.

pub(crate) mod proof_bearing_scalar;
pub(crate) mod scalar_array;
pub mod scalar_demand;
pub(crate) mod scalar_field_requirements;
pub(crate) mod scalar_leaf_schema;
pub(crate) mod scalar_leaf_semantics;
pub(crate) mod scalar_observations;
pub mod scalar_ordering;
pub(crate) mod scalar_representation_view;
pub(crate) mod unchanged_scalar;
