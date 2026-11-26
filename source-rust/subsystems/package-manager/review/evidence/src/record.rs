/*
Stable, inert review evidence vocabulary and canonical policy-row model emitted
by compiler-owned projection.

This module owns the value shapes (`PackageReviewEvidence`,
`PackagePolicyBaseline`, `PackagePolicyRow`, and all domain, contract, callable,
provider, and representation records) together with their structural invariant
validators. It never inspects live compiler state, encodes persistence bytes, or
grants package admission.
*/

mod accessors;
mod authority;
mod baseline;
pub use baseline::PackagePolicyComponentAssumption;
pub use baseline::{PackagePolicyBaseline, PackagePolicyNativeBinding};
mod authority_expressions;
mod behavior_policy;
pub(crate) use behavior_policy::{PackagePolicyCapabilityFlow, PackagePolicyProgressPremise};
pub use behavior_policy::{
    PackagePolicyCrash, PackagePolicyCrashGuard, PackagePolicyCrashRoute,
    PackagePolicyInferredCrash, PackagePolicyMutation, PackagePolicyTermination,
};
mod calling;
mod conformances;
mod contracts;
mod data;
mod domains;
mod identity;
mod semantic_bindings;
mod semantic_policy;
pub use identity::write_framed_identity;
pub use semantic_bindings::SemanticBindingReviewCandidate;
pub use semantic_policy::{
    PackagePolicySemanticDependency, PackagePolicySemanticDependencyConsumer,
};
pub(crate) mod package;
mod policy_rows;
/// Exact source units reported by a review. An absent inventory cannot support cache reuse.
#[derive(Debug)]
pub struct PackageReviewSourceFacts {
    pub consumed_units: Option<Vec<source_provenance::ConsumedSourceUnit>>,
}
mod public_policy;
mod representation;
mod restricted_build;
pub use policy_rows::{
    PACKAGE_POLICY_ROW_VERSION, PackagePolicyRow, PackagePolicyRowKind, PackagePolicyRowLimits,
    PackagePolicyRowUsage,
};
pub(crate) mod signatures;
mod target_context;
pub(in crate::record) use public_policy::validation;
pub use public_policy::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyMachineParameterContract, PackagePolicyPublicApi, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackagePolicyTypeAlias, PackagePolicyTypeParameter,
    PackagePolicyTypeParameterKind,
};
pub(crate) use public_policy::{
    PackagePolicyMachineParameterSignature, PackagePolicyOperatorShape,
};
pub use target_context::PackagePolicyTargetContext;

pub use authority::PackageReviewDangerousAuthorityClass;
pub(crate) use authority::{
    PackageReviewBooleanExpression, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthoritySlack, PackageReviewIeeeFloatComparisonKind,
    PackageReviewIntegerBinaryKind, PackageReviewIntegerComparisonKind,
    PackageReviewIntegerLiteral, PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange,
    PackageReviewPrimitiveType, PackageReviewProgressSubject, PackageReviewRuntimeIndex,
    PackageReviewScalarExpression, PackageReviewStructuralParameterField,
    PackageReviewStructuralPredicatePathSegment,
};
pub use authority::{
    PackageReviewCrashCause, PackageReviewCrashInterface, PackageReviewInstallationReach,
    PackageReviewWriteFrameCompleteness,
};
pub(crate) use calling::{
    PackagePolicyCallbackBinder, PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackInlineField, PackagePolicyCallbackLayout,
    PackagePolicyCallbackLayoutApplication, PackagePolicyCallbackMaterialization,
    PackagePolicyCallingOpaqueUse, PackagePolicyEntryStack, PackagePolicyPreemption,
};
pub use calling::{
    PackagePolicyCallbacks, PackagePolicyCallingParameter, PackagePolicyCallingPlan,
    PackagePolicyEntryControl, PackagePolicyMachineRegime, PackagePolicyMachineState,
    PackagePolicyMachineStateSet, PackagePolicyNativeParameter, PackagePolicyNativeParameterOrigin,
    PackagePolicyPhysicalCallingContract, PackagePolicyStatePlan,
};
pub(crate) use conformances::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};
pub use contracts::PackageReviewLayoutQueryKind;
pub use contracts::{
    PackageReviewArithmeticDomain, PackageReviewCastForm, PackageReviewFloatLiteral,
    PackageReviewReferenceAccess,
};
pub(crate) use contracts::{
    PackageReviewAtomicLoadOrdering, PackageReviewConstructorField,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractStaticArgument, PackageReviewContractUnaryOperator, PackageReviewMatchArm,
    PackageReviewOperatorRealization, PackageReviewPropositionApplication,
    PackageReviewPropositionBinder, PackageReviewPropositionBinderArgument,
    PackageReviewPropositionBinderArgumentKind, PackageReviewPropositionBinderKind,
    PackageReviewPropositionBinderValue, PackageReviewPropositionEvidence,
    PackageReviewPropositionParameterApplication, PackageReviewPropositionShape,
    PackageReviewPublicPropositionBody,
};
pub use contracts::{
    PackageReviewByteSequencePredicate, PackageReviewCallableSupply, PackageReviewConstMeaning,
    PackageReviewConstShape, PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewEvidenceInterface,
    PackageReviewEvidenceRequirement, PackageReviewOperatorCoordinate,
    PackageReviewResultCaseIdentity,
};
pub use contracts::{PackageReviewCallableContract, PackageReviewSynchronousInvocation};
pub use data::{
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataProperties,
};
pub use domains::PackageReviewDomainEstablishmentRoute;
pub(crate) use domains::{
    PackageReviewDomainClassification, PackageReviewDomainEstablishmentKind,
    PackageReviewDomainSemanticRole,
};
pub use identity::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewServiceIdentity,
};
pub use identity::{
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewToolchainSourceIdentity,
};
pub(crate) use package::{
    PackagePolicyBoundaryApplicationDemand, PackagePolicyCallableConformance,
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate,
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
    PackageReviewBoundaryApplicationArgument, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority, PackageReviewSymbolicBoundaryApplicationArgument,
};
pub use package::{
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization, PackagePolicyCallableRole, PackagePolicyCallables,
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicyServiceAuthority, PackagePolicyServiceReachDependency,
    PackagePolicyServiceSignature, PackageReviewBoundaryApplication,
    PackageReviewCheckedServiceReach, PackageReviewCompilerIntrinsicExecution,
    PackageReviewProviderGrantSelectorKind, PackageReviewSelectedInstallationReach,
};
pub use package::{
    PackagePolicyCallable, PackagePolicySelectedProviders, PackagePolicyServiceMethod,
    PackagePolicyTerminalPermissions,
};
pub use package::{PackagePolicyTerminalPermission, PackagePolicyTerminalService};
pub use representation::PackagePolicyRepresentation;
pub(crate) use representation::{
    PackagePolicyRepresentationAvailability, PackagePolicyRepresentationDemand,
    PackagePolicyRepresentationSelection, PackageReviewBoundaryCallingPolicy,
    PackageReviewBoundaryShapeClass, PackageReviewMachineRegister,
    PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement,
};
pub use representation::{
    PackageReviewBoundaryShape, PackageReviewBoundaryShapeField, PackageReviewBoundaryShapeGraph,
    PackageReviewBoundaryValueClass, PackageReviewBoundaryValueLocation,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewIndirectPointerLocation, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile, PackageReviewSystemVEightbyteClass,
};
pub use restricted_build::PackagePolicyRestrictedBuildRequest;
pub(crate) use restricted_build::{
    PackagePolicyEvaluationSponsorLimits, PackagePolicyFilesystemSponsorLimits,
    PackagePolicyRestrictedBuildBounds, PackagePolicyRestrictedBuildGrant,
    PackagePolicyRestrictedBuildGrantRoot, PackagePolicyRestrictedBuildOperation,
};

pub use signatures::{
    PackagePolicyEvaluatedBindingProducer, PackageReviewConformanceBound,
    PackageReviewSignatureParameter, PackageReviewTypeIdentity,
};
pub(crate) use signatures::{
    PackagePolicyExternalBinding, PackagePolicyExternalCallableSignature,
    PackagePolicyExternalExecutableSupply, PackagePolicyExternalRequirement,
    PackageReviewExternalCallableParameter, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewRequirementRealization,
};
pub use signatures::{
    PackageReviewBorrowOrigin, PackageReviewForeignLocator, PackageReviewParameterQualification,
    PackageReviewReturnedBorrowOrigin, PackageReviewTraitCompositionKind, PackageReviewTraitParent,
};
