//! Inverse of the complete structural signature vocabulary, without receipts.

use super::Error;
use super::expressions::static_argument;
use super::identity::{nominal, type_identity};
use super::reader::Reader;
use crate::record::{PackageReviewConformanceBound, PackageReviewDataProperties};

pub(in crate::encoding::recovery) fn conformance_bound(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConformanceBound, Error> {
    let binder_ordinal = reader.option(|reader| reader.u32())?;
    let subject_parameter = reader.u32()?;
    let (selected_conformance, selected_lifetime_arguments, selected_arguments, selected_subject) =
        match reader.byte()? {
            0 => (None, Vec::new(), Vec::new(), None),
            1 => (
                Some(nominal(reader)?),
                reader.sequence(4, |reader| reader.u32())?,
                reader.sequence(1, static_argument)?,
                Some(static_argument(reader)?),
            ),
            _ => return Err(Error::InvalidTag),
        };
    Ok(PackageReviewConformanceBound {
        binder_ordinal,
        subject_parameter,
        selected_conformance,
        selected_lifetime_arguments,
        selected_arguments,
        selected_subject,
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
