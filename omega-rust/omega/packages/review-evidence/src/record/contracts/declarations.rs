use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewCrashRoute;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewOperatorCoordinate;
use crate::record::PackageReviewSignatureParameter;
use crate::record::PackageReviewTypeIdentity;
use crate::record::PackageReviewTypeParameter;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConstShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) declared_type: PackageReviewTypeIdentity,
    pub(crate) canonical_value_encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewOperatorShape {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) is_boundary: bool,
    pub(crate) spelling: Option<language_core::OperatorSpelling>,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackageReviewCrashRoute>,
}

impl PackageReviewConstShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn declared_type(&self) -> &PackageReviewTypeIdentity {
        &self.declared_type
    }
}
