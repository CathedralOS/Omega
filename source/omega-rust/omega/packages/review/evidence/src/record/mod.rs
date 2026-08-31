//! Stable, inert evidence vocabulary emitted by compiler-owned projection.
//!
//! This module owns the review shapes and canonical-row model. It does not
//! inspect compiler state, encode persistence bytes, or make admission policy.

mod authority;
mod authority_expressions;
mod contracts;
mod data;
mod domains;
mod identity;
pub(crate) mod package;
mod representation;
mod rows;
mod signatures;

pub use authority::{
    PackageReviewBooleanExpression, PackageReviewCapabilityFlow, PackageReviewCrash,
    PackageReviewCrashCall, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewCrashPredicate, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewCrashSite, PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewIeeeFloatComparisonKind,
    PackageReviewInstallationReach, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewIntegerLiteral,
    PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange, PackageReviewMutation,
    PackageReviewPermissionClaim, PackageReviewPermissionSource, PackageReviewPrimitiveType,
    PackageReviewProgressPremise, PackageReviewProgressSubject, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
    PackageReviewTermination, PackageReviewWriteFrameCompleteness,
};
pub use contracts::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableContract, PackageReviewCallableRole,
    PackageReviewCallableSupply, PackageReviewCastForm, PackageReviewCollectionViewOperation,
    PackageReviewConstShape, PackageReviewConstructorField, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractEvidenceArgument,
    PackageReviewContractEvidenceTerm, PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewContractKind, PackageReviewContractOperatorMeaning,
    PackageReviewContractStaticArgument, PackageReviewContractUnaryOperator,
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement, PackageReviewFloatLiteral,
    PackageReviewOperatorCoordinate, PackageReviewOperatorRealization, PackageReviewOperatorShape,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
    PackageReviewReferenceAccess, PackageReviewResultCaseIdentity,
    PackageReviewSynchronousInvocation,
};
pub use data::{
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataProperties, PackageReviewDataShape,
};
pub use domains::{
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewDomainShape,
};
pub use identity::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewToolchainSourceIdentity,
};
pub use package::{
    CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageCallableReview,
    CheckedPackageProviderFamilyCoordinateReview, CheckedPackageProviderFamilyReview,
    CheckedPackageProviderReview, CheckedPackageProviderRowIdentity,
    CheckedPackageReviewProjection, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationRealizationRole, PackageReviewCheckedServiceReach,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach, PackageReviewSelectedProviderGrant,
};
pub use representation::{PackageReviewRepresentationTcb, PackageReviewRepresentationTcbKind};
pub use rows::{
    PackageReviewCanonicalRow, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCanonicalRowSource, PackageReviewSourceLocation, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole, PackageReviewSyntheticSourceKind,
};
pub use signatures::{
    PackageReviewCallableConformance, PackageReviewCallableParameter,
    PackageReviewConformanceBound, PackageReviewConformanceShape, PackageReviewConformanceSubject,
    PackageReviewExternalBinding, PackageReviewExternalCallableParameter,
    PackageReviewExternalCallableSignature, PackageReviewExternalExecutableSupply,
    PackageReviewExternalRequirement, PackageReviewExternalStaticParameter,
    PackageReviewMachineParameterContract, PackageReviewMachineParameterSignature,
    PackageReviewMachineParameterValue, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewTraitCompositionKind,
    PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};
