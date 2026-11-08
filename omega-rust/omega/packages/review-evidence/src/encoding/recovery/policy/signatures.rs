//! Inverse of the complete structural signature vocabulary, without receipts.

use super::Error;
use super::identity::{nominal, type_identity};
use super::reader::Reader;
use crate::record::{PackageReviewConformanceBound, PackageReviewDataProperties};

pub(in crate::encoding::recovery) fn conformance_bound(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConformanceBound, Error> {
    Ok(PackageReviewConformanceBound {
        binder_ordinal: reader.u32()?,
        trait_identity: nominal(reader)?,
        trait_lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
    })
}

pub(in crate::encoding::recovery) fn data_properties(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataProperties, Error> {
    use language_semantics::Multiplicity;
    let multiplicity = super::tag!(reader => {
        0 => Multiplicity::Unrestricted,
        1 => Multiplicity::Affine,
        2 => Multiplicity::Linear,
    });
    let carry = reader.option(super::selected_providers::service::carry)?;
    Ok(PackageReviewDataProperties {
        multiplicity,
        carry,
    })
}
