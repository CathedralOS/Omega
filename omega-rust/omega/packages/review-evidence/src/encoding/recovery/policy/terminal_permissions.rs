//! Bounded receipt-free permissions, preserving complete service schemas.

use super::{
    Error, PackagePolicyRecoveryLimits,
    identity::{nominal, package},
    public_api::type_parameter,
    reader::Reader,
    selected_providers::service_method,
};
use crate::encoding::{
    PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION, TERMINAL_PERMISSION_POLICY_MAGIC,
};
use crate::record::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};
use effects::{TerminalAuthorityClass, TerminalAuthorityDisposition};

impl PackagePolicyTerminalPermissions {
    super::recover_canonical_component!(
        TERMINAL_PERMISSION_POLICY_MAGIC,
        PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION,
        policy,
        validate,
    );
}

pub(in crate::encoding::recovery) fn policy(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyTerminalPermissions, Error> {
    Ok(PackagePolicyTerminalPermissions {
        package: package(reader)?,
        target: super::selected_providers::target(reader)?,
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
    })
}

pub(in crate::encoding::recovery) fn disposition(
    reader: &mut Reader<'_>,
) -> Result<TerminalAuthorityDisposition, Error> {
    let mut previous = None;
    let classes = reader.sequence(1, |reader| {
        let tag = reader.byte()?;
        let class = TerminalAuthorityClass::ALL
            .into_iter()
            .find(|class| class.canonical_tag() == tag)
            .ok_or(Error::InvalidTag)?;
        if previous.is_some_and(|prior| prior >= class) {
            return Err(Error::NonCanonicalEncoding);
        }
        previous = Some(class);
        Ok(class)
    })?;
    TerminalAuthorityDisposition::try_from_canonical_classes(classes)
        .map_err(|_| Error::NonCanonicalEncoding)
}
