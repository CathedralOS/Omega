use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitRequirementParameter {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
    pub(crate) is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCallableParameter {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
    pub(crate) is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCallableConformance {
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) requirement_identity: PackageReviewNominalIdentity,
    pub(crate) requirement_lifetime_partition: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) alias: Option<String>,
}

/// Closed structural identity of executable code supplied outside Omega.
/// String fields are foreign ABI identifiers, not package-authored policy or
/// capability classifications.

impl PackageReviewCallableConformance {
    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub const fn requirement_identity(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_identity
    }

    pub fn requirement_lifetime_partition(&self) -> &[u32] {
        &self.requirement_lifetime_partition
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

impl PackageReviewCallableParameter {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    pub const fn is_self(&self) -> bool {
        self.is_self
    }
}

impl PackageReviewTraitRequirementParameter {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    pub const fn is_self(&self) -> bool {
        self.is_self
    }
}
