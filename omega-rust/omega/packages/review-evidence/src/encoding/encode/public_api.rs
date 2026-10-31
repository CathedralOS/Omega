//! Full policy declarations share the enclosing baseline writer and budgets.

use crate::record::PackagePolicyPublicApi;
pub(super) mod declarations;
mod signatures;
use super::{PackageReviewEncodingError, encoder::Encoder};
pub(in crate::encoding) use declarations::conformance_shape;
pub(super) use signatures::{formal, type_parameter};

pub(in crate::encoding) fn public_api(
    encoder: &mut Encoder,
    api: &PackagePolicyPublicApi,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence_field("traits", &api.traits, declarations::trait_shape)?;
    encoder.field("conformances", |encoder| {
        encoder.sequence(&api.conformances, declarations::conformance_shape)
    })?;
    encoder.sequence_field("domains", &api.domains, declarations::domain_shape)?;
    encoder.field("propositions", |encoder| {
        encoder.sequence(
            &api.propositions,
            super::values::declarations::encode_proposition_shape,
        )
    })?;
    encoder.field("consts", |encoder| {
        encoder.sequence(&api.consts, super::values::declarations::encode_const_shape)
    })?;
    encoder.sequence_field("operators", &api.operators, declarations::operator_shape)?;
    encoder.sequence_field("data", &api.data, declarations::data_shape)
}
