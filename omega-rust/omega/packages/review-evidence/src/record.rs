//! Stable, inert evidence vocabulary emitted by compiler-owned projection.
//!
//! This module owns the review shapes and canonical-row model. It does not
//! inspect compiler state, encode persistence bytes, or make admission policy.

mod authority;
mod baseline;
pub use baseline::PackagePolicyBaseline;
mod authority_expressions;
mod behavior_policy;
pub use behavior_policy::{
    PackagePolicyCapabilityFlow, PackagePolicyCrash, PackagePolicyCrashGuard,
    PackagePolicyCrashRoute, PackagePolicyInferredCrash, PackagePolicyMutation,
    PackagePolicyProgressPremise, PackagePolicyTermination,
};
mod calling;
mod conformances;
mod contracts;
mod data;
mod domains;
mod identity;
mod semantic_policy;
pub(crate) use identity::write_framed_identity;
pub use semantic_policy::{
    PackagePolicySemanticDependency, PackagePolicySemanticDependencyConsumer,
};
pub(crate) mod package;
mod policy_rows;
mod public_policy;
mod representation;
mod restricted_build;
mod rows;
pub use policy_rows::{
    PACKAGE_POLICY_ROW_VERSION, PackagePolicyRow, PackagePolicyRowKind, PackagePolicyRowLimits,
    PackagePolicyRowUsage,
};
mod signatures;
pub(in crate::record) use public_policy::validation;
pub use public_policy::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyMachineParameterContract, PackagePolicyMachineParameterSignature,
    PackagePolicyOperatorShape, PackagePolicyPublicApi, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
};

pub use authority::{
    PackageReviewBooleanExpression, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewCrashPredicate, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewIeeeFloatComparisonKind,
    PackageReviewInstallationReach, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewIntegerLiteral,
    PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange, PackageReviewPrimitiveType,
    PackageReviewProgressPremise, PackageReviewProgressSubject, PackageReviewRuntimeIndex,
    PackageReviewScalarExpression, PackageReviewStructuralParameterField,
    PackageReviewStructuralPredicatePathSegment, PackageReviewTermination,
    PackageReviewWriteFrameCompleteness,
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
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewToolchainSourceIdentity,
};
pub use package::{
    CheckedPackageBoundaryApplicationRealizationReview, PackagePolicyBoundaryApplicationDemand,
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization, PackagePolicyCallable, PackagePolicyCallableConformance,
    PackagePolicyCallableRole, PackagePolicyCallables, PackagePolicyProviderBinding,
    PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderFamily,
    PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicySelectedProviders, PackagePolicyServiceAuthority, PackagePolicyServiceMethod,
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
    PackagePolicyServiceReachDependency, PackagePolicyServiceSignature,
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions,
    PackagePolicyTerminalService, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationArgument, PackageReviewBoundaryApplicationRealization,
    PackageReviewCheckedServiceReach, PackageReviewCompilerIntrinsicExecution,
    PackageReviewProviderFamilyCoverage, PackageReviewProviderGrantSelectorKind,
    PackageReviewProviderSelectionAuthority, PackageReviewSelectedInstallationReach,
    PackageReviewSymbolicBoundaryApplicationArgument,
};
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
    PackageReviewRepresentationTargetProfile, PackageReviewSystemVEightbyteClass,
};
pub use restricted_build::{
    PackagePolicyEvaluationSponsorLimits, PackagePolicyFilesystemSponsorLimits,
    PackagePolicyRestrictedBuildBounds, PackagePolicyRestrictedBuildGrant,
    PackagePolicyRestrictedBuildGrantRoot, PackagePolicyRestrictedBuildOperation,
    PackagePolicyRestrictedBuildRequest,
};
pub use rows::{
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
};
pub use signatures::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalCallableSignature, PackagePolicyExternalExecutableSupply,
    PackagePolicyExternalRequirement, PackageReviewCallableParameter,
    PackageReviewConformanceBound, PackageReviewConformanceShape,
    PackageReviewEvaluatedBindingUsage, PackageReviewEvaluatedImport,
    PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewExternalCallableParameter, PackageReviewForeignLocator,
    PackageReviewMachineParameterContract, PackageReviewMachineParameterSignature,
    PackageReviewMachineParameterValue, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewRequirementRealization,
    PackageReviewTraitCompositionKind, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitRequirementParameter, PackageReviewTraitShape, PackageReviewTypeIdentity,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};
