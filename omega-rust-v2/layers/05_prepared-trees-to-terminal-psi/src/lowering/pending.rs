//! Hand a package to its driver while some domains still wait for an access plan.
//!
//! Lowering cannot say who may issue `domain i32::Locked access Rule`: that is
//! what running `Rule::plan` returns, and lowering runs nothing. So
//! [`crate::lower_pending_policies`] lowers everything else and returns the
//! package without that domain's row. The row comes back beside it with no
//! issuer, together with each cast that adds the domain to a value. The driver
//! authenticates the executed plans and completes the pending domain rows.
//!
//! Nothing here can turn the package into a published one without the driver's
//! decision for every listed domain.
use crate::{DeclarationId, LoweredDomain, LoweredPackage, LoweringError, MachineId, Site, TypeId};

/// The issuer field of a domain row whose access plan is unanswered. It has one
/// value and says nothing, so the row cannot be read as `Unrestricted`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IssuersPending;

/// One cast that adds a pending domain to a value that lacks it, such as
/// `x as +Locked` in `machine issue(x: i32) -> i32 in Locked`. A formal, an
/// argument or a result that already carries the domain is not listed, and
/// neither is `x as -Locked`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyIntroduction {
    /// The body in the returned package whose edge adds the domain.
    pub machine: MachineId,
    /// That machine's Psi identity. Two instances of one generic machine share it.
    pub declaration: DeclarationId,
    /// The cast expression.
    pub site: Site,
}

impl PolicyIntroduction {
    /// The refusal for this cast when `domain`'s plan admits no issuer.
    pub fn refusal(
        &self,
        domain: &PendingPolicyDomain,
    ) -> LoweringError {
        LoweringError::new(
            &self.site,
            format!(
                "machine `{}` cannot introduce domain `{}`: its access plan admits no issuer",
                self.declaration.path, domain.row.declaration.path
            ),
        )
    }
}

/// A domain that selects an access policy, lowered without the plan's answer.
#[derive(Debug, Eq, PartialEq)]
pub struct PendingPolicyDomain {
    /// The source identity, which is `AccessPolicySelection::domain`.
    pub source: omega_resolved::DeclarationId,
    /// The `access Rule` clause that selects the policy.
    pub selection: Site,
    /// The row the package will publish once its issuer is known.
    pub row: LoweredDomain<TypeId, IssuersPending>,
    /// Every cast that adds this domain, ordered by machine and then by site.
    /// Empty when the package only forwards or erases the qualification.
    pub introductions: Vec<PolicyIntroduction>,
}

/// A lowered package and the domains it does not yet publish.
#[derive(Debug, Eq, PartialEq)]
pub struct PendingPolicyPackage {
    package: LoweredPackage,
    domains: Vec<PendingPolicyDomain>,
}

impl PendingPolicyPackage {
    pub(crate) fn new(
        package: LoweredPackage,
        domains: Vec<PendingPolicyDomain>,
    ) -> Self {
        Self { package, domains }
    }

    /// Every domain of the package that selects an access policy, in the order
    /// `lower_package` meets and refuses them: by source file, then by position.
    pub fn domains(&self) -> &[PendingPolicyDomain] {
        &self.domains
    }

    /// The package without the rows of its pending domains, and those domains.
    /// Its type table and bodies already name them, so it is not a complete
    /// package until each row is written with an issuer.
    pub fn into_parts(self) -> (LoweredPackage, Vec<PendingPolicyDomain>) {
        (self.package, self.domains)
    }
}
