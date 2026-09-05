//! Full policy declarations share the enclosing baseline writer and budgets.

mod declarations;
mod signatures;
use super::{PackageReviewEncodingError, encoder::Encoder};
use crate::record::*;
pub(in crate::encoding) use declarations::conformance_shape;
#[cfg(test)]
pub(in crate::encoding) use signatures::machine_contract;
pub(super) use signatures::type_parameter;

pub(in crate::encoding) fn public_api(
    encoder: &mut Encoder,
    api: &PackagePolicyPublicApi,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("traits", |encoder| {
        encoder.sequence(&api.traits, declarations::trait_shape)
    })?;
    encoder.field("conformances", |encoder| {
        encoder.sequence(&api.conformances, declarations::conformance_shape)
    })?;
    encoder.field("domains", |encoder| {
        encoder.sequence(&api.domains, declarations::domain_shape)
    })?;
    encoder.field("propositions", |encoder| {
        encoder.sequence(
            &api.propositions,
            super::values::declarations::encode_proposition_shape,
        )
    })?;
    encoder.field("consts", |encoder| {
        encoder.sequence(&api.consts, super::values::declarations::encode_const_shape)
    })?;
    encoder.field("operators", |encoder| {
        encoder.sequence(&api.operators, declarations::operator_shape)
    })?;
    encoder.field("data", |encoder| {
        encoder.sequence(&api.data, declarations::data_shape)
    })
}
