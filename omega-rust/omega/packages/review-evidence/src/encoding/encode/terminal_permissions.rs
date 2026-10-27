//! Exact service-schema permission meaning under one aggregate writer budget.

use super::{
    encoder::{Encoder, encode_canonical_component},
    public_api::type_parameter as encode_type_parameter,
    selected_providers::encode_service_method,
    values::identity::encode_nominal,
};
use crate::encoding::{
    PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION, PackageReviewEncodingError,
    TERMINAL_PERMISSION_POLICY_MAGIC,
};
use crate::record::{PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions};
use effects::TerminalAuthorityClass;

impl PackagePolicyTerminalPermissions {
    encode_canonical_component!(
        TERMINAL_PERMISSION_POLICY_MAGIC,
        PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION,
        policy,
    );
}

pub(super) fn policy(
    encoder: &mut Encoder,
    policy: &PackagePolicyTerminalPermissions,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("package", |encoder| {
        encoder.package_identity(policy.package);
        Ok(())
    })?;
    encoder.field("target", |encoder| {
        encoder.string(policy.target.identity().as_str())
    })?;
    encoder.field("services", |encoder| {
        encoder.sequence(&policy.services, |encoder, service| {
            encoder.field_by("service", &service.service, encode_nominal)?;
            encoder.field("static_parameters", |encoder| {
                encoder.sequence(&service.static_parameters, encode_type_parameter)
            })?;
            encoder.field("lifetime_parameter_count", |encoder| {
                encoder.u32(service.lifetime_parameter_count);
                Ok(())
            })?;
            encoder.field("methods", |encoder| {
                encoder.sequence(&service.methods, encode_service_method)
            })?;
            encoder.field("permissions", |encoder| {
                encoder.sequence(&service.permissions, |encoder, permission_value| {
                    permission(encoder, permission_value)
                })
            })
        })
    })
}

pub(super) fn permission(
    encoder: &mut Encoder,
    permission: &PackagePolicyTerminalPermission,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by("requirement", &permission.requirement, encode_nominal)?;
    encoder.field("permitted", |encoder| {
        encoder.sequence(permission.permitted.classes(), |encoder, class| {
            let name = match class {
                TerminalAuthorityClass::FilesystemContentRead => "filesystem_content_read",
                TerminalAuthorityClass::FilesystemContentWrite => "filesystem_content_write",
                TerminalAuthorityClass::FilesystemMetadataQuery => "filesystem_metadata_query",
                TerminalAuthorityClass::DirectoryEnumeration => "directory_enumeration",
                TerminalAuthorityClass::FilesystemNamespaceMutation => {
                    "filesystem_namespace_mutation"
                }
                TerminalAuthorityClass::FilesystemMetadataMutation => {
                    "filesystem_metadata_mutation"
                }
                TerminalAuthorityClass::ProcessOutput => "process_output",
                TerminalAuthorityClass::ProcessTermination => "process_termination",
                TerminalAuthorityClass::MachineControl => "machine_control",
                TerminalAuthorityClass::PortIo => "port_io",
                TerminalAuthorityClass::InterruptControl => "interrupt_control",
                TerminalAuthorityClass::InterruptEntry => "interrupt_entry",
                TerminalAuthorityClass::RootMemoryAccess => "root_memory_access",
                TerminalAuthorityClass::ProcessInput => "process_input",
            };
            encoder.tag(name, class.canonical_tag());
            Ok(())
        })
    })
}
