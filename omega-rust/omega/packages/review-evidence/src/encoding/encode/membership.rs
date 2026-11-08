// Source-graph package membership verification across retained semantic package owners without admission or declaration claims.

mod framing;
mod membership_accounting;
#[cfg(test)]
mod tests;
mod visitor;

use crate::record::PackagePolicyBaseline;
pub(crate) use membership_accounting::PackagePolicyMembershipUsage;
pub use membership_accounting::{
    MalformedIdentityExcerpt, PackagePolicyMembershipError, PackagePolicyMembershipFailure,
    PackagePolicyMembershipLimits,
};
use semantic_vocabulary::PackageKeyIdentity;

pub(super) trait Observer {
    fn package(&mut self, package: PackageKeyIdentity) -> Result<(), PackagePolicyMembershipError>;
    fn type_identity(&mut self, identity: &str) -> Result<(), PackagePolicyMembershipError>;
    fn nominal_path(&mut self, path: &str) -> Result<(), PackagePolicyMembershipError>;
}

impl PackagePolicyBaseline {
    pub fn validate_package_membership(
        &self,
        contains: impl FnMut(PackageKeyIdentity) -> bool,
        limits: PackagePolicyMembershipLimits,
    ) -> Result<PackagePolicyMembershipUsage, PackagePolicyMembershipFailure> {
        let mut visitor = visitor::Visitor::new(contains, limits);
        let mut encoder = super::encoder::Encoder::policy_membership(&mut visitor);
        let result = super::baseline::framed_policy(&mut encoder, self);
        let membership_error = encoder.membership_error();
        drop(encoder);
        if let Some(error) = membership_error {
            return Err(PackagePolicyMembershipFailure::new(
                error,
                visitor.unreadable_identity(),
            ));
        }
        result.map_err(|_| PackagePolicyMembershipError::InvalidPolicy)?;
        Ok(visitor.usage())
    }
}
