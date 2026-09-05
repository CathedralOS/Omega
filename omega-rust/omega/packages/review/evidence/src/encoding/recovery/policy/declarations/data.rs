use super::super::{Error, identity::type_identity, reader::Reader};
#[cfg(test)]
use super::super::{
    contracts::contract_fact,
    identity::nominal,
    signatures::{data_properties, type_parameter},
};
use crate::record::*;

#[cfg(test)]
pub(in crate::encoding::recovery::policy) fn data_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataShape, Error> {
    Ok(PackageReviewDataShape {
        identity: nominal(reader)?,
        kind: match reader.byte()? {
            0 => PackageReviewDataKind::Ordinary,
            1 => PackageReviewDataKind::Quotient {
                carrier: type_identity(reader)?,
                relation: nominal(reader)?,
            },
            _ => return Err(Error::InvalidTag),
        },
        supply: match reader.byte()? {
            0 => psi_language_semantics::DataSupplyMode::CheckedShape,
            1 => psi_language_semantics::DataSupplyMode::BoundaryOpaque,
            _ => return Err(Error::InvalidTag),
        },
        lifetime_parameter_count: reader.usize()?,
        type_parameters: reader.sequence(3, type_parameter)?,
        properties: data_properties(reader)?,
        zero_gated: reader.boolean()?,
        invariants: reader.sequence(2, contract_fact)?,
        retired_identities: reader.sequence(8, |reader| reader.u64())?,
        members: reader.sequence(19, data_member)?,
    })
}

pub(in crate::encoding::recovery::policy) fn data_member(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataMember, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewDataMember::Field(data_field(reader)?),
        1 => PackageReviewDataMember::Variant {
            identity: reader.option(|reader| reader.u64())?,
            name: reader.string()?,
            payload: reader.sequence(18, data_field)?,
            retired_payload_identities: reader.sequence(8, |reader| reader.u64())?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn data_field(reader: &mut Reader<'_>) -> Result<PackageReviewDataField, Error> {
    Ok(PackageReviewDataField {
        identity: reader.option(|reader| reader.u64())?,
        name: reader.string()?,
        relevance: match reader.byte()? {
            0 => psi_language_core::BindingRelevance::Relevant,
            1 => psi_language_core::BindingRelevance::Erased,
            _ => return Err(Error::InvalidTag),
        },
        type_identity: type_identity(reader)?,
    })
}
