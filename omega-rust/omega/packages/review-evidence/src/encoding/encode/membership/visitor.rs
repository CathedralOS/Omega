// Bounded package-owner visitor bridging `Observer` and `TypeIdentityPackageOwnerVisitor` while retaining the innermost unreadable identity excerpt.

use super::{
    MalformedIdentityExcerpt, Observer, PackagePolicyMembershipError as Error,
    PackagePolicyMembershipLimits as Limits, PackagePolicyMembershipUsage as Usage,
};
use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, malformed_identity_offset,
    visit_type_identity_package_owners,
};
use semantic_vocabulary::PackageKeyIdentity;

pub(super) struct Visitor<Contains> {
    contains: Contains,
    limits: Limits,
    usage: Usage,
    depth: usize,
    failure: Option<Error>,
    unreadable: Option<MalformedIdentityExcerpt>,
}

impl<Contains: FnMut(PackageKeyIdentity) -> bool> Visitor<Contains> {
    pub(super) fn new(contains: Contains, limits: Limits) -> Self {
        Self {
            contains,
            limits: limits.bounded(),
            usage: Usage::default(),
            depth: 0,
            failure: None,
            unreadable: None,
        }
    }
    pub(super) fn usage(&self) -> Usage {
        self.usage
    }
    pub(super) fn unreadable_identity(&mut self) -> Option<MalformedIdentityExcerpt> {
        self.unreadable.take()
    }

    pub(super) fn nested(
        &mut self,
        visit: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.enter_node()?;
        let result = visit(self);
        self.depth -= 1;
        result
    }

    fn enter_node(&mut self) -> Result<(), Error> {
        if self.usage.identity_nodes >= self.limits.maximum_identity_nodes {
            return Err(Error::IdentityNodeLimitExceeded);
        }
        if self.depth >= self.limits.maximum_depth {
            return Err(Error::NestingLimitExceeded);
        }
        self.usage.identity_nodes += 1;
        self.depth += 1;
        Ok(())
    }

    pub(super) fn runtime(&mut self, identity: &str) -> Result<(), Error> {
        visit_type_identity_package_owners(identity, self).map_err(|error| {
            if let Some(failure) = self.failure {
                return failure;
            }
            match error {
                TypeIdentityVisitError::AllocationFailed => Error::AllocationFailed,
                TypeIdentityVisitError::MalformedIdentity
                | TypeIdentityVisitError::UnsupportedEmbeddedName
                | TypeIdentityVisitError::Unadmitted(_) => {
                    self.keep_unreadable("runtime type identity", identity, || {
                        malformed_identity_offset(identity)
                    });
                    Error::MalformedIdentity
                }
                TypeIdentityVisitError::ResourceLimitExceeded
                | TypeIdentityVisitError::UnknownPackage => Error::InvalidPolicy,
            }
        })
    }

    fn keep_unreadable(
        &mut self,
        field: &'static str,
        identity: &str,
        stopped: impl FnOnce() -> Option<usize>,
    ) {
        if self.unreadable.is_none() {
            self.unreadable = Some(MalformedIdentityExcerpt::new(field, identity, stopped()));
        }
    }

    fn explain(
        &mut self,
        result: Result<(), Error>,
        field: &'static str,
        identity: &str,
    ) -> Result<(), Error> {
        if result == Err(Error::MalformedIdentity) {
            self.keep_unreadable(field, identity, || None);
        }
        result
    }

    fn bridge(&mut self, result: Result<(), Error>) -> Result<(), TypeIdentityVisitError> {
        result.map_err(|error| {
            if self.failure.is_none() {
                self.failure = Some(error);
            }
            match error {
                Error::UnknownPackage { .. } => TypeIdentityVisitError::UnknownPackage,
                Error::OwnedBytesLimitExceeded
                | Error::IdentityNodeLimitExceeded
                | Error::NestingLimitExceeded => TypeIdentityVisitError::ResourceLimitExceeded,
                Error::AllocationFailed => TypeIdentityVisitError::AllocationFailed,
                _ => TypeIdentityVisitError::MalformedIdentity,
            }
        })
    }
}

impl<Contains: FnMut(PackageKeyIdentity) -> bool> Observer for Visitor<Contains> {
    fn package(&mut self, package: PackageKeyIdentity) -> Result<(), Error> {
        self.nested(|visitor| {
            if (visitor.contains)(package) {
                Ok(())
            } else {
                Err(Error::UnknownPackage { package })
            }
        })
    }
    fn type_identity(&mut self, identity: &str) -> Result<(), Error> {
        let result = self.visit_type(identity);
        self.explain(result, "type identity", identity)
    }
    fn nominal_path(&mut self, path: &str) -> Result<(), Error> {
        let result = self.visit_name(path);
        self.explain(result, "nominal path", path)
    }
}

impl<Contains: FnMut(PackageKeyIdentity) -> bool> TypeIdentityPackageOwnerVisitor
    for Visitor<Contains>
{
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
        let result = self.enter_node();
        self.bridge(result)
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }
    fn reserve(&mut self, owned_bytes: usize) -> Result<(), TypeIdentityVisitError> {
        let result = self
            .usage
            .owned_bytes
            .checked_add(owned_bytes)
            .filter(|total| *total <= self.limits.maximum_owned_bytes)
            .ok_or(Error::OwnedBytesLimitExceeded)
            .map(|total| self.usage.owned_bytes = total);
        self.bridge(result)
    }
    fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
        let result = PackageKeyIdentity::from_digest(digest)
            .ok_or(Error::MalformedIdentity)
            .and_then(|package| self.package(package));
        self.bridge(result)
    }
    fn embedded_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError> {
        let result = self.visit_name(name);
        let result = self.explain(result, "embedded name", name);
        self.bridge(result)
    }
}
