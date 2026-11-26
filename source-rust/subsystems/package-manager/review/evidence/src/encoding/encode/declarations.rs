// Canonical encoders for declaration-shaped evidence: conformance bounds, dependency and trust rows, trait surfaces, semantic domains, and concrete data/type machinery.

mod data;
mod dependency_trust;
mod domains;
mod traits;

use super::PackageReviewEncodingError;
use super::encoder::Encoder;
pub(crate) use super::type_identity as encode_type_identity;
use super::values::identity::encode_nominal;
use crate::record::PackageReviewConformanceBound;
pub(crate) use dependency_trust::{
    calling_policy_tag, encode_boundary_shape_graph, encode_machine_register,
    encode_opaque_occurrence, encode_representation_target, encode_value_placement,
};

#[allow(
    unused_imports,
    reason = "retain every established declaration encoder entry point"
)]
pub(crate) use data::{
    encode_data_field, encode_data_member, encode_data_properties, encode_optional_u64,
    encode_relevance,
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

// Canonical encoding for conformance bounds in declaration signatures.

pub(crate) fn encode_conformance_bound(
    encoder: &mut Encoder,
    bound: &PackageReviewConformanceBound,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32_field("binder_ordinal", bound.binder_ordinal)?;
    encoder.field_by("trait_identity", &bound.trait_identity, encode_nominal)?;
    encoder.field("trait_lifetime_arguments", |encoder| {
        encoder.sequence(&bound.trait_lifetime_arguments, |encoder, argument| {
            encoder.u32_field("argument", *argument)?;
            Ok(())
        })
    })?;
    encoder.sequence_field("arguments", &bound.arguments, encode_type_identity)
}
