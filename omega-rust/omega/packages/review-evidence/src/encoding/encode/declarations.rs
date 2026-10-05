//! Canonical encoders for declaration-shaped evidence.
//!
//! The map follows the persisted evidence families: conformance identity,
//! dependency and trust rows, trait surfaces, semantic domains, and concrete
//! data/type machinery. Each leaf owns the exact byte order and tags for its
//! family; this entrance keeps their established crate-local call surface.

mod conformances;
mod data;
mod dependency_trust;
mod domains;
mod traits;

pub(crate) use dependency_trust::{
    encode_boundary_shape_graph, encode_opaque_occurrence, encode_representation_target,
};

pub(crate) use dependency_trust::{
    calling_policy_tag, encode_machine_register, encode_value_placement,
};

pub(crate) use conformances::encode_conformance_bound;
#[allow(
    unused_imports,
    reason = "retain every established declaration encoder entry point"
)]
pub(crate) use data::{
    encode_data_field, encode_data_member, encode_data_properties, encode_optional_u64,
    encode_relevance, encode_type_identity,
};
#[allow(
    unused_imports,
    reason = "retain every established declaration encoder entry point"
)]
pub(crate) use dependency_trust::{
    encode_dangerous_authority, encode_dangerous_authority_slack, semantic_dependency_kind_tag,
};
#[allow(
    unused_imports,
    reason = "retain every established declaration encoder entry point"
)]
pub(crate) use domains::{encode_domain_alias_atom, encode_domain_establishment_route};
#[allow(
    unused_imports,
    reason = "retain every established declaration encoder entry point"
)]
pub(crate) use traits::encode_trait_parent;
