//! Exact service-schema permissions, not exercised authority or admission.

mod generics;

use super::{PackagePolicyServiceMethod, provider_policy::validate_service_methods};
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
    crate::record::accessors::record_accessors!(package, target);
    pub fn services(&self) -> &[PackagePolicyTerminalService] {
        &self.services
    }

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
    crate::record::accessors::record_accessors!(static_parameters);
    pub fn service(&self) -> &PackageReviewNominalIdentity {
        &self.service
    }

    pub const fn lifetime_parameter_count(&self) -> u32 {
        self.lifetime_parameter_count
    }
    pub fn methods(&self) -> &[PackagePolicyServiceMethod] {
        &self.methods
    }
    pub fn permissions(&self) -> &[PackagePolicyTerminalPermission] {
        &self.permissions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalPermission {
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) permitted: TerminalAuthorityDisposition,
}

impl PackagePolicyTerminalPermission {
    crate::record::accessors::record_accessors!(requirement);
    pub fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }
}
