//! Complete callable policy, with no private derivation coordinates.

mod behavior;
mod row;
pub(super) use behavior::{crash_route, termination};
pub(in crate::encoding) use row::encode_callable;

use super::{
    declarations::{encode_conformance_bound, encode_type_identity},
    encoder::Encoder,
    public_api::type_parameter as encode_type_parameter,
    values::{
        contracts::encode_callable_contract,
        declarations::encode_operator_coordinate,
        effects::{encode_installation_reach, encode_synchronous_invocation},
        identity::{encode_nominal, encode_supply},
    },
};
use crate::encoding::{
    CALLABLE_POLICY_MAGIC, PACKAGE_CALLABLE_POLICY_VERSION, PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableConformance, PackagePolicyCallableRole,
    PackagePolicyCallables, PackageReviewCheckedServiceReach,
};

impl PackagePolicyCallables {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(CALLABLE_POLICY_MAGIC);
        encoder.u16(PACKAGE_CALLABLE_POLICY_VERSION);
        policy(&mut encoder, self)?;
        encoder.finish()
    }
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
    encoder.field("trait", |encoder| {
        encode_nominal(encoder, &conformance.trait_identity)
    })?;
    encoder.field("requirement", |encoder| {
        encode_nominal(encoder, &conformance.requirement_identity)
    })?;
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
