use super::{
    Encoder, MAGIC, PACKAGE_REVIEW_ENCODING_VERSION, PackageReviewEncodingError,
    PackageReviewEncodingLimits, encode_conformance_shape, encode_dangerous_authority,
    encode_dangerous_authority_slack, encode_data_shape, encode_domain_shape,
    encode_representation_tcb, encode_semantic_dependency, encode_trait_shape,
};
use crate::encoding::values::{
    encode_callable, encode_const_shape, encode_external_executable_supply, encode_operator_shape,
    encode_proposition_shape, encode_provider, encode_provider_family,
};
use crate::model::CheckedPackageReviewProjection;

pub(crate) fn encode(
    review: &CheckedPackageReviewProjection,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    encode_with_limits(review, PackageReviewEncodingLimits::default())
}

pub(crate) fn encode_with_limits(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    let mut encoder = Encoder::bounded(limits.maximum_review_bytes);
    encoder.fixed_bytes(MAGIC);
    encoder.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    encoder.package_identity(review.package);
    encoder.string(review.target.target_name())?;
    encoder.sequence(&review.public_traits, encode_trait_shape)?;
    encoder.sequence(&review.public_conformances, encode_conformance_shape)?;
    encoder.sequence(&review.public_domains, encode_domain_shape)?;
    encoder.sequence(&review.public_propositions, encode_proposition_shape)?;
    encoder.sequence(&review.public_consts, encode_const_shape)?;
    encoder.sequence(&review.public_operators, encode_operator_shape)?;
    encoder.sequence(&review.public_data, encode_data_shape)?;
    encoder.sequence(&review.representation_tcb, encode_representation_tcb)?;
    encoder.sequence(&review.semantic_dependencies, encode_semantic_dependency)?;
    encoder.sequence(&review.callables, encode_callable)?;
    encoder.sequence(
        &review.external_executable_supply,
        encode_external_executable_supply,
    )?;
    encoder.sequence(&review.dangerous_authorities, encode_dangerous_authority)?;
    encoder.sequence(
        &review.dangerous_authority_slack,
        encode_dangerous_authority_slack,
    )?;
    encoder.sequence(&review.selected_providers, encode_provider)?;
    encoder.sequence(&review.selected_provider_families, encode_provider_family)?;
    encoder.finish()
}
