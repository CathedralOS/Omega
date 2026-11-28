// Stable type, callable, trait, conformance, and external-supply signatures.

use super::PackageReviewNominalIdentity;

mod callables;
mod external_policy;
pub(crate) mod external_supply;
mod signature_vocabulary;
mod traits;

pub(crate) use callables::PackageReviewRequirementRealization;

pub use external_policy::PackagePolicyEvaluatedBindingProducer;
pub(crate) use external_policy::{
    PackagePolicyExternalBinding, PackagePolicyExternalCallableSignature,
    PackagePolicyExternalExecutableSupply, PackagePolicyExternalRequirement,
};

pub(crate) use external_supply::PackageReviewExternalCallableParameter;
pub use external_supply::PackageReviewForeignLocator;

pub(crate) use signature_vocabulary::PackageReviewPropositionParameterSignature;
#[cfg(any(test, feature = "full-policy-recovery"))]
pub(crate) use signature_vocabulary::PackageReviewPropositionParameterValue;
pub use signature_vocabulary::{
    PackageReviewConformanceBound, PackageReviewSignatureParameter, PackageReviewTypeIdentity,
};

pub use traits::{
    PackageReviewBorrowOrigin, PackageReviewParameterQualification,
    PackageReviewReturnedBorrowOrigin,
};
pub use traits::{PackageReviewTraitCompositionKind, PackageReviewTraitParent};
