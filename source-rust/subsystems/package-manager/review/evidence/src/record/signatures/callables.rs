// Checked body top-level requirement realization associations (matched positionally without authored arguments).

use crate::record::PackageReviewNominalIdentity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewRequirementRealization {
    pub(crate) requirement_identity: PackageReviewNominalIdentity,
    pub(crate) alias: Option<String>,
}

impl PackageReviewRequirementRealization {}
