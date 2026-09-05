use super::super::{
    Error,
    contracts::contract_fact,
    declarations::data_member,
    identity::{nominal, type_identity},
    reader::Reader,
    signatures::data_properties,
};
use super::signatures::type_parameter;
use crate::record::*;

pub(super) fn data_shape(reader: &mut Reader<'_>) -> Result<PackagePolicyDataShape, Error> {
    Ok(PackagePolicyDataShape {
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
