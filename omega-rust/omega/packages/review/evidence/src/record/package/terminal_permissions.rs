//! Exact service-schema permissions, not exercised authority or admission.

mod generics;
#[cfg(test)]
pub(crate) use generics::write_service_parameter_identity;

use super::{PackagePolicyServiceMethod, provider_policy::validate_service_methods};
use crate::record::{
    PackagePolicyTypeParameter, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
use omega_effects::TerminalAuthorityDisposition;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyTerminalPermissions {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) services: Vec<PackagePolicyTerminalService>,
}

impl PackagePolicyTerminalPermissions {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }
    pub const fn target(&self) -> TargetProfile {
        self.target
    }
    pub fn services(&self) -> &[PackagePolicyTerminalService] {
        &self.services
    }

    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if self
            .services
            .windows(2)
            .any(|pair| pair[0].service >= pair[1].service)
        {
            return Err("terminal permission services repeat or reorder identities");
        }
        for service in &self.services {
            validate_service_methods(&service.methods, &service.service, self.target)?;
            generics::validate(service)?;
            if service.permissions.is_empty()
                || service
                    .permissions
                    .windows(2)
                    .any(|pair| pair[0].requirement >= pair[1].requirement)
            {
                return Err("terminal permission service has empty or noncanonical permissions");
            }
            for permission in &service.permissions {
                if permission.requirement.path.is_empty()
                    || permission.requirement.owner == PackageReviewNominalOwner::Unresolved
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
    pub fn service(&self) -> &PackageReviewNominalIdentity {
        &self.service
    }
    pub fn static_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.static_parameters
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
    pub fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
    pub fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }
}
