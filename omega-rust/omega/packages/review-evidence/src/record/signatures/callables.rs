use crate::record::PackageReviewNominalIdentity;

/// A checked body's exact top-level requirement association. Unlike trait
/// conformance, this edge has no authored arguments: static and lifetime
/// telescopes are matched positionally by conformance checking. The complete
/// requirement overload remains distinct from an operator coordinate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewRequirementRealization {
    pub(crate) requirement_identity: PackageReviewNominalIdentity,
    pub(crate) alias: Option<String>,
}

impl PackageReviewRequirementRealization {}
