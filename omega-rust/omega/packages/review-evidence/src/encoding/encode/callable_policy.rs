//! Complete callable policy, with no private derivation coordinates.

mod behavior;
mod row;
pub(super) use behavior::{crash_route, termination};
pub(in crate::encoding) use row::encode_callable;

use super::declarations::encode_type_identity;
use super::encoder::{Encoder, encode_canonical_component};
use super::public_api::type_parameter as encode_type_parameter;
use super::values::identity::encode_nominal;
use crate::encoding::{
    CALLABLE_POLICY_MAGIC, PACKAGE_CALLABLE_POLICY_VERSION, PackageReviewEncodingError,
};
use crate::record::{PackagePolicyCallableConformance, PackagePolicyCallables};

impl PackagePolicyCallables {
    encode_canonical_component!(
        CALLABLE_POLICY_MAGIC,
        PACKAGE_CALLABLE_POLICY_VERSION,
        policy,
    );
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicyCallables,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("package", |encoder| {
        encoder.package_identity(policy.package);
        Ok(())
    })?;
    encoder.field("target", |encoder| {
        encoder.string(policy.target.identity().as_str())
    })?;
    encoder.field("callables", |encoder| {
        encoder.sequence(&policy.callables, encode_callable)
    })
}

pub(super) fn encode_callable_conformance(
    encoder: &mut Encoder,
    conformance: &PackagePolicyCallableConformance,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("trait", &conformance.trait_identity, encode_nominal)?;
    encoder.field_by(
        "requirement",
        &conformance.requirement_identity,
        encode_nominal,
    )?;
    encoder.field("requirement_lifetime_partition", |encoder| {
        encoder.sequence(
            &conformance.requirement_lifetime_partition,
            |encoder, ordinal| {
                encoder.u32(*ordinal);
                Ok(())
            },
        )
    })?;
    encoder.field("trait_lifetime_arguments", |encoder| {
        encoder.sequence(&conformance.trait_lifetime_arguments, |encoder, ordinal| {
            encoder.u32(*ordinal);
            Ok(())
        })
    })?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&conformance.arguments, encode_type_identity)
    })?;
    encoder.field("alias", |encoder| {
        encoder.option(conformance.alias.as_deref(), |encoder, alias| {
            encoder.string(alias)
        })
    })
}
