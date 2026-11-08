//! Shared semantic dependency, authority, and representation encoding.

mod authority;
mod geometry;
mod placements;
mod registers;

pub(crate) use authority::{
    encode_dangerous_authority, encode_dangerous_authority_slack, semantic_dependency_kind_tag,
};
pub(crate) use geometry::{
    encode_boundary_shape_graph, encode_opaque_occurrence, encode_representation_target,
};
pub(crate) use placements::encode_value_placement;
pub(crate) use registers::{calling_policy_tag, encode_machine_register};
