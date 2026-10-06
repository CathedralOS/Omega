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
    use language_semantics::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, Multiplicity,
    };
    let multiplicity = match reader.byte()? {
        0 => Multiplicity::Unrestricted,
        1 => Multiplicity::Affine,
        2 => Multiplicity::Linear,
        _ => return Err(Error::InvalidTag),
    };
    let carry = reader.option(|reader| {
        Ok(CarryPolicy {
            suspension: match reader.byte()? {
                0 => CarrySuspension::Forbidden,
                1 => CarrySuspension::Allowed,
                _ => return Err(Error::InvalidTag),
            },
            cpu: match reader.byte()? {
                0 => CarryCpu::Origin,
                1 => CarryCpu::Any,
                _ => return Err(Error::InvalidTag),
            },
            host_thread: match reader.byte()? {
                0 => CarryHostThread::Origin,
                1 => CarryHostThread::Any,
                _ => return Err(Error::InvalidTag),
            },
            address: match reader.byte()? {
                0 => CarryAddress::Stable,
                1 => CarryAddress::Movable,
                _ => return Err(Error::InvalidTag),
            },
        })
    })?;
    Ok(PackageReviewDataProperties {
        multiplicity,
        carry,
    })
}
