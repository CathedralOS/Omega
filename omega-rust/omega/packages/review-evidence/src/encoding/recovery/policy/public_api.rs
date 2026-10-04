//! Normalized public API meaning, sharing its enclosing baseline budget.

pub(in crate::encoding::recovery) mod data;
pub(in crate::encoding::recovery) mod domains;
pub(in crate::encoding::recovery) mod signatures;
pub(in crate::encoding::recovery) mod traits;
pub(in crate::encoding::recovery) use signatures::type_parameter;
pub(in crate::encoding::recovery) use traits::conformance_shape;

use super::{Error, declarations, reader::Reader};
use crate::record::PackagePolicyPublicApi;

pub(in crate::encoding::recovery) fn public_api(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyPublicApi, Error> {
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
