//! Bounded recovery of the complete inert conformance application component.

use super::{
    Error, PackagePolicyRecoveryLimits,
    identity::{nominal, type_identity},
    reader::Reader,
};
use crate::encoding::{CONFORMANCE_POLICY_MAGIC, PACKAGE_CONFORMANCE_POLICY_VERSION};
use crate::record::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};

impl PackagePolicyClosedConformanceApplication {
    /// Decode semantic policy without source access or reconstructing a proof.
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(CONFORMANCE_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_CONFORMANCE_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let application = Self {
            declaration: nominal(&mut reader)?,
            lifetime_arguments: reader.sequence(4, Reader::u32)?,
            type_arguments: reader.sequence(8, type_identity)?,
            const_arguments: reader.sequence(1, const_argument)?,
            machine_arguments: reader.sequence(41, nominal)?,
            subject: reader.option(type_identity)?,
            trait_identity: nominal(&mut reader)?,
            trait_lifetime_arguments: reader.sequence(4, Reader::u32)?,
            trait_arguments: reader.sequence(8, type_identity)?,
            rows: reader.sequence(164, |reader| {
                Ok(PackagePolicyConformanceRow {
                    declaring_trait: nominal(reader)?,
                    requirement: nominal(reader)?,
                    realization_machine: nominal(reader)?,
                    realization_state: nominal(reader)?,
                })
            })?,
        };
        reader.finish()?;
        reader.canonical_scratch(bytes.len())?;
        if application
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(application)
    }
}

fn const_argument(reader: &mut Reader<'_>) -> Result<PackagePolicyConformanceConstArgument, Error> {
    Ok(match reader.byte()? {
        0 => PackagePolicyConformanceConstArgument::Evaluated {
            parameter_carrier: type_identity(reader)?,
            declared_carrier: type_identity(reader)?,
            canonical_value_encoding: reader.string()?,
        },
        1 => PackagePolicyConformanceConstArgument::CallerBinder {
            parameter_carrier: type_identity(reader)?,
            binder: nominal(reader)?,
            binder_carrier: type_identity(reader)?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

#[cfg(test)]
mod tests;
