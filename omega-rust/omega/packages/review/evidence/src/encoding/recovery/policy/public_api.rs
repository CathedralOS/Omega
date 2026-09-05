//! Normalized public API meaning, sharing its enclosing baseline budget.

mod data;
mod domains;
mod signatures;
mod traits;
pub(super) use signatures::type_parameter;
pub(super) use traits::conformance_shape;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(super) use tests::fixture as row_fixture;

use super::{Error, declarations, reader::Reader};
use crate::record::PackagePolicyPublicApi;

pub(super) fn public_api(reader: &mut Reader<'_>) -> Result<PackagePolicyPublicApi, Error> {
    Ok(PackagePolicyPublicApi {
        traits: reader.sequence(1, traits::trait_shape)?,
        conformances: reader.sequence(1, traits::conformance_shape)?,
        domains: reader.sequence(1, domains::domain_shape)?,
        propositions: reader.sequence(1, declarations::proposition_shape)?,
        consts: reader.sequence(1, declarations::const_shape)?,
        operators: reader.sequence(1, traits::operator_shape)?,
        data: reader.sequence(1, data::data_shape)?,
    })
}
