//! Stable, inert evidence vocabulary emitted by compiler-owned projection.
//!
//! This module owns the review shapes and canonical-row model. It does not
//! inspect compiler state, encode persistence bytes, or make admission policy.

mod accessors;
mod authority;
mod baseline;
pub use baseline::PackagePolicyBaseline;
mod authority_expressions;
mod behavior_policy;
pub(crate) use behavior_policy::{
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
pub(crate) use semantic_policy::{
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
pub(crate) mod signatures;
mod target_context;
pub(in crate::record) use public_policy::validation;
pub(crate) use public_policy::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyMachineParameterContract, PackagePolicyMachineParameterSignature,
    PackagePolicyOperatorShape, PackagePolicyPublicApi, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
};
pub(crate) use target_context::PackagePolicyTargetContext;

pub use authority::PackageReviewDangerousAuthorityClass;
pub(crate) use authority::{
    PackageReviewBooleanExpression, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewCrashPredicate, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthoritySlack,
    PackageReviewIeeeFloatComparisonKind, PackageReviewInstallationReach,
    PackageReviewIntegerBinaryKind, PackageReviewIntegerComparisonKind,
    PackageReviewIntegerLiteral, PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange,
    PackageReviewPrimitiveType, PackageReviewProgressPremise, PackageReviewProgressSubject,
    PackageReviewRuntimeIndex, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
    PackageReviewTermination, PackageReviewWriteFrameCompleteness,
};
pub(crate) use calling::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallbacks, PackagePolicyCallingOpaqueUse, PackagePolicyCallingParameter,
    PackagePolicyCallingPlan, PackagePolicyEntryControl, PackagePolicyEntryStack,
    PackagePolicyMachineRegime, PackagePolicyMachineState, PackagePolicyMachineStateSet,
    PackagePolicyNativeParameter, PackagePolicyNativeParameterOrigin,
    PackagePolicyPhysicalCallingContract, PackagePolicyPreemption, PackagePolicyStatePlan,
};
pub(crate) use conformances::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};
pub(crate) use contracts::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCallableRole, PackageReviewCallableSupply,
    PackageReviewCastForm, PackageReviewConstShape, PackageReviewConstructorField,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewEvidenceInterface,
    PackageReviewEvidenceRequirement, PackageReviewFloatLiteral, PackageReviewMatchArm,
    PackageReviewOperatorCoordinate, PackageReviewOperatorRealization, PackageReviewOperatorShape,
    PackageReviewPropositionApplication, PackageReviewPropositionBinder,
    PackageReviewPropositionBinderArgument, PackageReviewPropositionBinderArgumentKind,
    PackageReviewPropositionBinderKind, PackageReviewPropositionBinderValue,
    PackageReviewPropositionEvidence, PackageReviewPropositionParameterApplication,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
    PackageReviewReferenceAccess, PackageReviewResultCaseIdentity,
};
pub use contracts::{PackageReviewCallableContract, PackageReviewSynchronousInvocation};
pub(crate) use data::{
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataProperties, PackageReviewDataShape,
};
pub(crate) use domains::{
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewDomainShape,
};
pub use identity::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
pub(crate) use identity::{
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewToolchainSourceIdentity,
};
pub(crate) use package::{
    CheckedPackageBoundaryApplicationRealizationReview, PackagePolicyBoundaryApplicationDemand,
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization, PackagePolicyCallableConformance, PackagePolicyCallableRole,
    PackagePolicyCallables, PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicyServiceAuthority, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute, PackagePolicyServiceReachDependency,
    PackagePolicyServiceSignature, PackagePolicyTerminalPermission, PackagePolicyTerminalService,
    PackageReviewBoundaryApplication, PackageReviewBoundaryApplicationArgument,
    PackageReviewBoundaryApplicationRealization, PackageReviewCheckedServiceReach,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach, PackageReviewSymbolicBoundaryApplicationArgument,
};
pub use package::{
    PackagePolicyCallable, PackagePolicySelectedProviders, PackagePolicyServiceMethod,
    PackagePolicyTerminalPermissions,
};
pub use representation::PackagePolicyRepresentation;
pub(crate) use representation::{
    PackagePolicyRepresentationAvailability, PackagePolicyRepresentationDemand,
    PackagePolicyRepresentationSelection, PackageReviewBoundaryCallingPolicy,
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
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
pub(crate) use restricted_build::{
    PackagePolicyEvaluationSponsorLimits, PackagePolicyFilesystemSponsorLimits,
    PackagePolicyRestrictedBuildBounds, PackagePolicyRestrictedBuildGrant,
    PackagePolicyRestrictedBuildGrantRoot, PackagePolicyRestrictedBuildOperation,
    PackagePolicyRestrictedBuildRequest,
};
pub(crate) use rows::{
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
};
pub(crate) use signatures::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalCallableSignature, PackagePolicyExternalExecutableSupply,
    PackagePolicyExternalRequirement, PackageReviewConformanceBound, PackageReviewConformanceShape,
    PackageReviewEvaluatedImport, PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewExternalCallableParameter, PackageReviewForeignLocator,
    PackageReviewMachineParameterContract, PackageReviewMachineParameterSignature,
    PackageReviewPropositionParameterSignature, PackageReviewPropositionParameterValue,
    PackageReviewRequirementRealization, PackageReviewSignatureParameter,
    PackageReviewTraitCompositionKind, PackageReviewTraitParent, PackageReviewTraitRequirement,
    PackageReviewTraitShape, PackageReviewTypeIdentity, PackageReviewTypeParameter,
    PackageReviewTypeParameterKind,
};
