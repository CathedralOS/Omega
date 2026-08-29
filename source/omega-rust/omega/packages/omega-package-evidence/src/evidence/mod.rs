//! Stable, inert evidence vocabulary emitted by compiler-owned projection.
//!
//! This module owns the review shapes and canonical-row model. It does not
//! inspect compiler state, encode persistence bytes, or make admission policy.

mod api;
mod authority;
mod contracts;
mod identity;
pub(crate) mod package;
mod rows;
mod signatures;

pub use api::{
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember, PackageReviewDataShape,
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewDomainShape,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewRepresentationTcb,
};
pub use authority::{
    PackageReviewCapabilityFlow, PackageReviewCrash, PackageReviewCrashCall,
    PackageReviewCrashInterface, PackageReviewCrashPredicate, PackageReviewCrashRoute,
    PackageReviewCrashRouteGuard, PackageReviewCrashSite, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthorityClass, PackageReviewDangerousAuthoritySlack,
    PackageReviewInstallationReach, PackageReviewMutation, PackageReviewPermissionClaim,
    PackageReviewPermissionSource, PackageReviewProgressPremise, PackageReviewProgressSubject,
    PackageReviewTermination,
};
pub use contracts::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableContract, PackageReviewCallableRole,
    PackageReviewCallableSupply, PackageReviewCastForm, PackageReviewConstShape,
    PackageReviewConstructorField, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewContractKind, PackageReviewContractOperatorMeaning,
    PackageReviewContractStaticArgument, PackageReviewContractUnaryOperator,
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement, PackageReviewFloatLiteral,
    PackageReviewOperatorCoordinate, PackageReviewOperatorRealization, PackageReviewOperatorShape,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderKind,
    PackageReviewPropositionBinderValue, PackageReviewPropositionEvidence,
    PackageReviewPropositionParameterApplication, PackageReviewPropositionShape,
    PackageReviewPublicPropositionBody, PackageReviewReferenceAccess,
    PackageReviewResultCaseIdentity, PackageReviewSynchronousInvocation,
};
pub use identity::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewToolchainSourceIdentity,
};
pub use package::{
    CheckedPackageCallableReview, CheckedPackageProviderFamilyCoordinateReview,
    CheckedPackageProviderFamilyExactApplicationReview, CheckedPackageProviderFamilyReview,
    CheckedPackageProviderReview, CheckedPackageProviderRowIdentity,
    CheckedPackageReviewProjection, PackageReviewCheckedServiceReach,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyApplicationCoverage,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach,
};
pub use rows::{
    PackageReviewCanonicalRow, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCanonicalRowSource, PackageReviewSourceLocation, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole, PackageReviewSyntheticSourceKind,
};
pub use signatures::{
    PackageReviewCallableConformance, PackageReviewCallableParameter,
    PackageReviewConformanceBound, PackageReviewConformanceShape, PackageReviewConformanceSubject,
    PackageReviewExternalBinding, PackageReviewExternalExecutableSupply,
    PackageReviewExternalRequirement, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewMachineParameterValue,
    PackageReviewPropositionParameterSignature, PackageReviewPropositionParameterValue,
    PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};
