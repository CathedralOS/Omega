//! Stable, inert evidence vocabulary emitted by compiler-owned projection.
//!
//! This module owns the review shapes and canonical-row model. It does not
//! inspect compiler state, encode persistence bytes, or make admission policy.

mod authority;
mod authority_expressions;
mod calling;
mod conformances;
mod contracts;
mod data;
mod domains;
mod identity;
pub(crate) use identity::write_framed_identity;
pub(crate) mod package;
mod quotients;
mod representation;
mod rows;
mod signatures;
mod terminal_authority;

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
pub use calling::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyCallingParameter,
    PackagePolicyCallingPlan, PackagePolicyEntryControl, PackagePolicyEntryStack,
    PackagePolicyMachineRegime, PackagePolicyMachineState, PackagePolicyMachineStateSet,
    PackagePolicyNativeParameter, PackagePolicyNativeParameterOrigin,
    PackagePolicyPhysicalCallingContract, PackagePolicyPreemption, PackagePolicyStatePlan,
};
pub use conformances::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};
pub use contracts::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableContract, PackageReviewCallableRole,
    PackageReviewCallableSupply, PackageReviewCastForm, PackageReviewCollectionViewOperation,
    PackageReviewConstShape, PackageReviewConstructorField, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractEntailmentAssumptionDischarge,
    PackageReviewContractEntailmentOpenObligation, PackageReviewContractEntailmentOpenReason,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewEvidenceInterface,
    PackageReviewEvidenceRequirement, PackageReviewFloatLiteral, PackageReviewOperatorCoordinate,
    PackageReviewOperatorRealization, PackageReviewOperatorShape,
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
    CheckedPackageBoundaryApplicationDemandReview,
    CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageCallableReview,
    CheckedPackageProviderFamilyCoordinateReview, CheckedPackageProviderFamilyReview,
    CheckedPackageProviderReview, CheckedPackageProviderRowIdentity,
    CheckedPackageReviewProjection, PackagePolicyProviderBinding,
    PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderFamily,
    PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicySelectedProviders, PackagePolicyServiceAuthority, PackagePolicyServiceMethod,
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
    PackagePolicyServiceSignature, PackagePolicyTerminalPermission,
    PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
    PackageReviewBoundaryApplication, PackageReviewBoundaryApplicationArgument,
    PackageReviewBoundaryApplicationRealization, PackageReviewBoundaryApplicationRealizationRole,
    PackageReviewCheckedServiceReach, PackageReviewCompilerIntrinsicExecution,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderGrantSelectorKind,
    PackageReviewProviderSelectionAuthority, PackageReviewSelectedInstallationReach,
    PackageReviewSelectedProviderGrant, PackageReviewSymbolicBoundaryApplicationArgument,
};
pub use quotients::NonExecutableQuotientPackageReview;
pub use representation::{
    PackagePolicyRepresentation, PackagePolicyRepresentationAvailability,
    PackagePolicyRepresentationDemand, PackagePolicyRepresentationSelection,
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryShape,
    PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewBoundaryValueShape, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile, PackageReviewRepresentationTcb,
    PackageReviewRepresentationTcbKind, PackageReviewSystemVEightbyteClass,
};
pub use rows::{
    PackageReviewCanonicalRow, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCanonicalRowSource, PackageReviewSourceLocation, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole, PackageReviewSyntheticSourceKind,
};
pub use signatures::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalExecutableSupply, PackageReviewCallableConformance,
    PackageReviewCallableParameter, PackageReviewConformanceBound, PackageReviewConformanceShape,
    PackageReviewConformanceSubject, PackageReviewEvaluatedBindingUsage,
    PackageReviewEvaluatedImport, PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewExternalCallableParameter, PackageReviewExternalCallableSignature,
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewExternalStaticParameter, PackageReviewForeignLocator,
    PackageReviewMachineParameterContract, PackageReviewMachineParameterSignature,
    PackageReviewMachineParameterValue, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewTraitCompositionKind,
    PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};
pub use terminal_authority::PackageReviewTerminalAuthorityPermission;
