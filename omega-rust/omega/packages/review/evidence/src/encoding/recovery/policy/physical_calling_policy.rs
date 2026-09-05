//! Recover an inert physical component, not validator or native custody.

mod placement;
mod state;
#[cfg(test)]
mod tests;

use super::{Error, PackagePolicyRecoveryLimits, reader::Reader};
use crate::encoding::{PACKAGE_PHYSICAL_CALLING_POLICY_VERSION, PHYSICAL_CALLING_POLICY_MAGIC};
use crate::record::{PackagePolicyPhysicalCallingContract, PackageReviewBoundaryCallingPolicy};

impl PackagePolicyPhysicalCallingContract {
    /// Bounded canonical format recovery. The result remains an incomplete
    /// policy component and cannot reconstruct a ValidatedBoundaryEntryPlan.
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(PHYSICAL_CALLING_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_PHYSICAL_CALLING_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let component = Self {
            policy: policy(&mut reader)?,
            parameters: reader.sequence(13, placement::value_placement)?,
            result: reader.option(placement::value_placement)?,
            ordinary_clobbers: reader.sequence(1, placement::register)?,
            stack_alignment: reader.u16()?,
            shadow_bytes: reader.u16()?,
            entry_control: state::entry_control(&mut reader)?,
            state: state::state_plan(&mut reader)?,
        };
        reader.finish()?;
        reader.canonical_scratch(bytes.len())?;
        if component
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(component)
    }
}

fn policy(reader: &mut Reader<'_>) -> Result<PackageReviewBoundaryCallingPolicy, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewBoundaryCallingPolicy::MicrosoftX64,
        1 => PackageReviewBoundaryCallingPolicy::SystemVAMD64,
        2 => PackageReviewBoundaryCallingPolicy::Aapcs64,
        3 => PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64,
        4 => PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64,
        _ => return Err(Error::InvalidTag),
    })
}
