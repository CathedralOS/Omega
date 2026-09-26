//! Stable type, callable, trait, conformance, and external-supply signatures.

use super::identity::PackageReviewNominalIdentity;

mod callables;
mod external_policy;
mod external_supply;
mod signature_vocabulary;
mod traits;

pub use callables::{
    PackageReviewCallableConformance, PackageReviewCallableParameter,
    PackageReviewTraitRequirementParameter,
};
pub use external_policy::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalCallableSignature, PackagePolicyExternalExecutableSupply,
    PackagePolicyExternalRequirement,
};
pub use external_supply::{
    PackageReviewEvaluatedBindingUsage, PackageReviewEvaluatedImport,
    PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewExternalCallableParameter, PackageReviewExternalCallableSignature,
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewExternalStaticParameter, PackageReviewForeignLocator,
};
pub use signature_vocabulary::{
    PackageReviewConformanceBound, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewMachineParameterValue,
    PackageReviewPropositionParameterSignature, PackageReviewPropositionParameterValue,
    PackageReviewTypeIdentity, PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};
pub use traits::{
    PackageReviewConformanceShape, PackageReviewConformanceSubject,
    PackageReviewTraitCompositionKind, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitShape,
};
