use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;

/// A checked body's exact top-level requirement association. Unlike trait
/// conformance, this edge has no authored arguments: static and lifetime
/// telescopes are matched positionally by conformance checking. The complete
/// requirement overload remains distinct from an operator coordinate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewRequirementRealization {
    pub(crate) requirement_identity: PackageReviewNominalIdentity,
    pub(crate) alias: Option<String>,
}

impl PackageReviewRequirementRealization {
    pub const fn requirement_identity(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_identity
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewTraitRequirementParameter {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCallableParameter {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewCallableParameter {
    crate::record::accessors::signature_parameter_accessors!();
}

impl PackageReviewTraitRequirementParameter {
    crate::record::accessors::signature_parameter_accessors!();
}
