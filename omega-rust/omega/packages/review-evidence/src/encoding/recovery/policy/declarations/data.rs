use super::super::{Error, identity::type_identity, reader::Reader};
use crate::record::{PackageReviewDataField, PackageReviewDataMember};

pub(in crate::encoding::recovery) fn data_member(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataMember, Error> {
    Ok(super::super::tag!(reader => {
        0 => PackageReviewDataMember::Field(data_field(reader)?),
        1 => PackageReviewDataMember::Variant {
            identity: reader.option(|reader| reader.u64())?,
            name: reader.string()?,
            payload: reader.sequence(18, data_field)?,
            retired_payload_identities: reader.sequence(8, |reader| reader.u64())?,
        },
    }))
}

pub(in crate::encoding::recovery) fn data_field(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataField, Error> {
    Ok(PackageReviewDataField {
        identity: reader.option(|reader| reader.u64())?,
        name: reader.string()?,
        relevance: super::super::tag!(reader => {
            0 => language_core::BindingRelevance::Relevant,
            1 => language_core::BindingRelevance::Erased,
        }),
        type_identity: type_identity(reader)?,
    })
}
