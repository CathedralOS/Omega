//! Exact service-schema permissions, not exercised authority or admission.

mod generics;

use super::{PackagePolicyServiceMethod, provider_policy::validate_service_methods};
use crate::record::accessors::field_projection_accessors;
use crate::record::validation::ordered_by;
use crate::record::{PackagePolicyTypeParameter, PackageReviewNominalIdentity};
use effects::TerminalAuthorityDisposition;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalPermissions {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) services: Vec<PackagePolicyTerminalService>,
}

impl PackagePolicyTerminalPermissions {
    field_projection_accessors!(copied package: semantic_vocabulary::PackageKeyIdentity);
    field_projection_accessors!(copied target: target::TargetProfile);
    field_projection_accessors!(sliced services: &[PackagePolicyTerminalService]);

    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        ordered_by(&self.services, |a, b| a.service.cmp(&b.service))?;
        for service in &self.services {
            validate_service_methods(&service.methods, &service.service, self.target)?;
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
    pub(crate) service: PackageReviewNominalIdentity,
    pub(crate) static_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) lifetime_parameter_count: u32,
    /// Complete schema in original declaration order, including unpermitted methods.
    pub(crate) methods: Vec<PackagePolicyServiceMethod>,
    pub(crate) permissions: Vec<PackagePolicyTerminalPermission>,
}

impl PackagePolicyTerminalService {
    field_projection_accessors!(sliced static_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(borrowed service: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied lifetime_parameter_count: u32);
    field_projection_accessors!(sliced methods: &[PackagePolicyServiceMethod]);
    field_projection_accessors!(sliced permissions: &[PackagePolicyTerminalPermission]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalPermission {
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) permitted: TerminalAuthorityDisposition,
}

impl PackagePolicyTerminalPermission {
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed permitted: &TerminalAuthorityDisposition);
}
