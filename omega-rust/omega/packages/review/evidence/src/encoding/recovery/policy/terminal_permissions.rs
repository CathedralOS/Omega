//! Bounded receipt-free permissions, preserving complete service schemas.

#[cfg(test)]
mod budgets;
#[cfg(test)]
mod generics;
#[cfg(test)]
mod tests;

use super::{
    Error, PackagePolicyRecoveryLimits,
    identity::{nominal, package},
    reader::Reader,
    selected_providers::service_method,
    signatures::type_parameter,
};
use crate::encoding::{
    PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION, TERMINAL_PERMISSION_POLICY_MAGIC,
};
use crate::record::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};
use omega_effects::{TerminalAuthorityClass, TerminalAuthorityDisposition};

impl PackagePolicyTerminalPermissions {
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(TERMINAL_PERMISSION_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let package = package(&mut reader)?;
        let target_identity = reader.string()?;
        let target = omega_target::TargetProfile::ALL
            .into_iter()
            .find(|target| target.identity().as_str() == target_identity)
            .ok_or(Error::InvalidValue)?;
        let policy = Self {
            package,
            target,
            services: reader.sequence(69, |reader| {
                Ok(PackagePolicyTerminalService {
                    service: nominal(reader)?,
                    static_parameters: reader.sequence(3, type_parameter)?,
                    lifetime_parameter_count: reader.u32()?,
                    methods: reader.sequence(1, service_method)?,
                    permissions: reader.sequence(49, |reader| {
                        Ok(PackagePolicyTerminalPermission {
                            requirement: nominal(reader)?,
                            permitted: disposition(reader)?,
                        })
                    })?,
                })
            })?,
        };
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

fn disposition(reader: &mut Reader<'_>) -> Result<TerminalAuthorityDisposition, Error> {
    let mut previous = None;
    let classes = reader.sequence(1, |reader| {
        use TerminalAuthorityClass as Class;
        let class = match reader.byte()? {
            0 => Class::FilesystemContentRead,
            1 => Class::FilesystemContentWrite,
            2 => Class::FilesystemMetadataQuery,
            3 => Class::DirectoryEnumeration,
            4 => Class::FilesystemNamespaceMutation,
            5 => Class::FilesystemMetadataMutation,
            6 => Class::ProcessOutput,
            7 => Class::ProcessTermination,
            8 => Class::MachineControl,
            9 => Class::PortIo,
            10 => Class::InterruptControl,
            11 => Class::InterruptEntry,
            12 => Class::RootMemoryAccess,
            13 => Class::ProcessInput,
            _ => return Err(Error::InvalidTag),
        };
        if previous.is_some_and(|prior| prior >= class) {
            return Err(Error::NonCanonicalEncoding);
        }
        previous = Some(class);
        Ok(class)
    })?;
    TerminalAuthorityDisposition::try_from_canonical_classes(classes)
        .map_err(|_| Error::NonCanonicalEncoding)
}
