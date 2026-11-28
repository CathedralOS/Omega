// Exact service-schema permissions (`PackagePolicyTerminalPermissions`),
// recording review policy rather than exercised authority or admission.

use crate::record::PackageReviewServiceIdentity;
#[path = "terminal_permissions_generics.rs"]
mod generics;

use super::{PackagePolicyServiceMethod, provider_policy::validate_service_methods};
use crate::record::accessors::field_projection_accessors;
use crate::record::validation::ordered_by;
use crate::record::{PackagePolicyTypeParameter, PackageReviewNominalIdentity};
use semantic_vocabulary::PackageKeyIdentity;
use semantic_vocabulary::TerminalAuthorityDisposition;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalPermissions {
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    pub services: Vec<PackagePolicyTerminalService>,
}

impl PackagePolicyTerminalPermissions {
    field_projection_accessors!(copied package: semantic_vocabulary::PackageKeyIdentity);
    field_projection_accessors!(copied target: target::TargetProfile);
    field_projection_accessors!(sliced services: &[PackagePolicyTerminalService]);

    pub(crate) fn has_network_authority(&self) -> bool {
        use semantic_vocabulary::TerminalAuthorityClass::{
            HostNetworkStack, OffMachineNetworkTransmit,
        };
        self.services.iter().flat_map(|s| &s.permissions).any(|p| {
            p.permitted
                .classes()
                .iter()
                .any(|class| matches!(class, HostNetworkStack | OffMachineNetworkTransmit))
        })
    }

    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        ordered_by(&self.services, |a, b| a.service.cmp(&b.service))?;
        for service in &self.services {
            service.service.validate_resolved()?;
            validate_service_methods(&service.methods, &service.service.declaration, self.target)?;
            generics::validate(service)?;
            if service.permissions.is_empty() {
                return Err("terminal permission service has no permissions");
            }
            ordered_by(&service.permissions, |a, b| {
                a.requirement.cmp(&b.requirement)
            })?;
            for permission in &service.permissions {
                if permission.requirement.validate_resolved().is_err()
                    || !service
                        .methods
                        .iter()
                        .any(|method| method.requirement == permission.requirement)
                {
                    return Err("terminal permission is detached from its complete service schema");
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalService {
    pub service: PackageReviewServiceIdentity,
    pub static_parameters: Vec<PackagePolicyTypeParameter>,
    pub lifetime_parameter_count: u32,
    pub methods: Vec<PackagePolicyServiceMethod>, // Complete schema in original declaration order, including unpermitted methods.
    pub permissions: Vec<PackagePolicyTerminalPermission>,
}

impl PackagePolicyTerminalService {
    field_projection_accessors!(sliced static_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(borrowed service: &PackageReviewServiceIdentity);

    field_projection_accessors!(copied lifetime_parameter_count: u32);
    field_projection_accessors!(sliced methods: &[PackagePolicyServiceMethod]);
    field_projection_accessors!(sliced permissions: &[PackagePolicyTerminalPermission]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalPermission {
    pub requirement: PackageReviewNominalIdentity,
    pub permitted: TerminalAuthorityDisposition,
}

impl PackagePolicyTerminalPermission {
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed permitted: &TerminalAuthorityDisposition);
}
