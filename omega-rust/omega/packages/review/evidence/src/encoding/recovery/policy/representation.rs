//! Bounded, source-independent recovery of complete representation policy.

mod availability;
#[cfg(test)]
mod availability_tests;
#[cfg(test)]
mod budgets;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod group_tests;

use super::{
    Error, PackagePolicyRecoveryLimits, calling_application, conformance,
    identity::{nominal, owner, package},
    reader::Reader,
};
use crate::encoding::{PACKAGE_REPRESENTATION_POLICY_VERSION, REPRESENTATION_POLICY_MAGIC};
use crate::record::{
    PackagePolicyRepresentation, PackagePolicyRepresentationDemand,
    PackagePolicyRepresentationSelection, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
};

impl PackagePolicyRepresentation {
    /// Recover policy without source access, evaluator replay, or acceptance.
    /// All nested applications use this one reader's aggregate budgets.
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(REPRESENTATION_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_REPRESENTATION_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let policy = policy(&mut reader)?;
        reader.finish()?;
        policy
            .validate_canonical_structure()
            .map_err(|_| Error::InvalidValue)?;
        reader.canonical_scratch(bytes.len())?;
        if policy
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(policy)
    }
}

pub(super) fn policy(reader: &mut Reader<'_>) -> Result<PackagePolicyRepresentation, Error> {
    Ok(PackagePolicyRepresentation {
        package: package(reader)?,
        target: calling_application::shapes::target(reader)?,
        declarations: reader.sequence(41, nominal)?,
        producer_availability: reader.sequence(1, availability::availability)?,
        selected_availability: reader.sequence(1, selection)?,
        demands: reader.sequence(1, |reader| {
            Ok(PackagePolicyRepresentationDemand {
                opaque: nominal(reader)?,
                calling: calling_application::application(reader)?,
            })
        })?,
    })
}

fn selection(reader: &mut Reader<'_>) -> Result<PackagePolicyRepresentationSelection, Error> {
    Ok(PackagePolicyRepresentationSelection {
        opaque: nominal(reader)?,
        carrier: nominal(reader)?,
        selection_owner: owner(reader)?,
        application: conformance::application(reader)?,
        origin: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
            _ => return Err(Error::InvalidTag),
        },
        lifecycle: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
            _ => return Err(Error::InvalidTag),
        },
        copy_disposition: match reader.byte()? {
            0 => PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
            1 => PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
            _ => return Err(Error::InvalidTag),
        },
    })
}
