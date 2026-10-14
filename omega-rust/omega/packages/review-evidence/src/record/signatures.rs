//! Stable type, callable, trait, conformance, and external-supply signatures.

use super::identity::PackageReviewNominalIdentity;

mod callables;
mod external_policy;
pub(crate) mod external_supply;
mod signature_vocabulary;
mod traits;

pub(crate) use callables::PackageReviewRequirementRealization;

pub(crate) use external_policy::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalCallableSignature, PackagePolicyExternalExecutableSupply,
    PackagePolicyExternalRequirement,
};

pub(crate) use external_supply::{
    PackageReviewEvaluatedImport, PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewExternalCallableParameter, PackageReviewForeignLocator,
};

pub(crate) use signature_vocabulary::{
    PackageReviewConformanceBound, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewSignatureParameter,
    PackageReviewTypeIdentity, PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};

pub(crate) use traits::{
    PackageReviewConformanceShape, PackageReviewTraitCompositionKind, PackageReviewTraitParent,
    PackageReviewTraitRequirement, PackageReviewTraitShape,
};
