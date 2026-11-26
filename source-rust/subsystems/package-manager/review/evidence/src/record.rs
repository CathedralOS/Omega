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
use crate::record::accessors::field_projection_accessors;
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
    PackageReviewCrashCause, PackageReviewCrashInterface, PackageReviewDangerousAuthorityClass,
    PackageReviewInstallationReach, PackageReviewWriteFrameCompleteness,
};
pub use baseline::{
    PackagePolicyBaseline, PackagePolicyComponentAssumption, PackagePolicyNativeBinding,
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
pub use contracts::{
    PackageReviewArithmeticDomain, PackageReviewByteSequencePredicate,
    PackageReviewCallableContract, PackageReviewCallableSupply, PackageReviewCastForm,
    PackageReviewConstMeaning, PackageReviewConstShape, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewContractKind, PackageReviewContractOperatorMeaning,
    PackageReviewEvidenceInterface, PackageReviewEvidenceRequirement, PackageReviewFloatLiteral,
    PackageReviewLayoutQueryKind, PackageReviewOperatorCoordinate, PackageReviewReferenceAccess,
    PackageReviewResultCaseIdentity, PackageReviewSynchronousInvocation,
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
pub(crate) use package::{
    PackagePolicyBoundaryApplicationDemand, PackagePolicyCallableConformance,
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate,
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
    PackageReviewBoundaryApplicationArgument, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority, PackageReviewSymbolicBoundaryApplicationArgument,
};
pub use package::{
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization, PackagePolicyCallable, PackagePolicyCallableRole,
    PackagePolicyCallables, PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackagePolicyProviderPlan, PackagePolicyProviderRow, PackagePolicySelectedProviders,
    PackagePolicyServiceAuthority, PackagePolicyServiceMethod, PackagePolicyServiceReachDependency,
    PackagePolicyServiceSignature, PackagePolicyTerminalPermission,
    PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
    PackageReviewBoundaryApplication, PackageReviewCheckedServiceReach,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderGrantSelectorKind,
    PackageReviewSelectedInstallationReach,
};
pub use policy_rows::{
    PACKAGE_POLICY_ROW_VERSION, PackagePolicyRow, PackagePolicyRowKind, PackagePolicyRowLimits,
    PackagePolicyRowUsage,
};
use provider_acceptance::AcceptedSemanticBinding;
use provider_plans::ServiceSchema;
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
pub use representation::{
    PackagePolicyRepresentation, PackageReviewBoundaryShape, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewBoundaryValueShape, PackageReviewIndirectPointerLocation,
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
    PackageReviewSystemVEightbyteClass,
};
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
pub use restricted_build::PackagePolicyRestrictedBuildRequest;
pub(crate) use restricted_build::{
    PackagePolicyEvaluationSponsorLimits, PackagePolicyFilesystemSponsorLimits,
    PackagePolicyRestrictedBuildBounds, PackagePolicyRestrictedBuildGrant,
    PackagePolicyRestrictedBuildGrantRoot, PackagePolicyRestrictedBuildOperation,
};
use semantic_vocabulary::PackageKeyIdentity;
pub use signatures::{
    PackagePolicyEvaluatedBindingProducer, PackageReviewBorrowOrigin,
    PackageReviewConformanceBound, PackageReviewForeignLocator,
    PackageReviewParameterQualification, PackageReviewReturnedBorrowOrigin,
    PackageReviewSignatureParameter, PackageReviewTraitCompositionKind, PackageReviewTraitParent,
    PackageReviewTypeIdentity,
};
pub(crate) use signatures::{
    PackagePolicyExternalBinding, PackagePolicyExternalCallableSignature,
    PackagePolicyExternalExecutableSupply, PackagePolicyExternalRequirement,
    PackageReviewExternalCallableParameter, PackageReviewPropositionParameterSignature,
    PackageReviewPropositionParameterValue, PackageReviewRequirementRealization,
};
use source_provenance::PackageSourceConsumptionCommitment;
mod authority_expressions;
mod calling;
mod conformances;
mod contracts;
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
pub(crate) mod signatures;

// Callable behavior meaning without implementation locations or proof receipts.
//
// `PackagePolicyInferredCrash::Unknown` indicates the checked local dependency
// graph did not yield a closed summary; `Complete` records a conservative,
// complete cause set rather than proof that each cause occurs.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCapabilityFlow {
    pub(crate) capability: PackageReviewNominalIdentity,
    pub(crate) kind: psi_effect_contracts::CapabilityFlowKind,
}

impl PackagePolicyCapabilityFlow {
    field_projection_accessors!(borrowed capability: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied kind: psi_effect_contracts::CapabilityFlowKind);
    pub(crate) fn compare_canonical(&self, other: &Self) -> std::cmp::Ordering {
        self.capability
            .cmp(&other.capability)
            .then_with(|| self.kind_tag().cmp(&other.kind_tag()))
    }
    fn kind_tag(&self) -> u8 {
        match self.kind {
            psi_effect_contracts::CapabilityFlowKind::Uses => 0,
            psi_effect_contracts::CapabilityFlowKind::Returns => 1,
            psi_effect_contracts::CapabilityFlowKind::Acquires => 2,
            psi_effect_contracts::CapabilityFlowKind::Stores => 3,
            psi_effect_contracts::CapabilityFlowKind::Derives => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyMutation {
    pub completeness: PackageReviewWriteFrameCompleteness,
    pub paths: Vec<String>,
}
impl PackagePolicyMutation {
    field_projection_accessors!(copied completeness: PackageReviewWriteFrameCompleteness);
    field_projection_accessors!(sliced paths: &[String]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyCrashGuard {
    Truth,
    Expression(PackageReviewContractExpression),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCrashRoute {
    pub cause: PackageReviewCrashCause,
    pub alternative_guards: Vec<PackagePolicyCrashGuard>,
}
impl PackagePolicyCrashRoute {
    field_projection_accessors!(copied cause: PackageReviewCrashCause);
    field_projection_accessors!(sliced alternative_guards: &[PackagePolicyCrashGuard]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyInferredCrash {
    Unknown,
    Complete {
        causes: Vec<PackageReviewCrashCause>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCrash {
    pub interface: PackageReviewCrashInterface,
    pub published: Vec<PackagePolicyCrashRoute>,
    pub structural_runtime_requirements: Option<Vec<PackageReviewBooleanExpression>>,
    pub inferred: PackagePolicyInferredCrash,
}
impl PackagePolicyCrash {
    field_projection_accessors!(copied interface: PackageReviewCrashInterface);
    field_projection_accessors!(sliced published: &[PackagePolicyCrashRoute]);
    pub fn structural_runtime_requirements(&self) -> Option<&[PackageReviewBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }
    field_projection_accessors!(borrowed inferred: &PackagePolicyInferredCrash);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: PackageReviewProgressSubject,
    pub(crate) projections: Vec<PackageReviewNominalIdentity>,
    pub(crate) establishment_routes: Vec<PackagePolicyServiceProgressRoute>,
}
impl PackagePolicyProgressPremise {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackagePolicyProgressPremise>,
    },
}

// Stable public-data evidence.
//
// `PackageReviewDataKind::Quotient` records the carrier family plus relation
// declaration; the proof implementation that licensed formation is not API
// identity. The properties record retains the declaration's multiplicity.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    pub identity: Option<u64>,
    pub name: String,
    pub relevance: language_core::BindingRelevance,
    pub type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataMember {
    Field(PackageReviewDataField),
    Variant {
        identity: Option<u64>,
        name: String,
        payload: Vec<PackageReviewDataField>,
        retired_payload_identities: Vec<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataKind {
    Ordinary,
    Quotient {
        carrier: PackageReviewTypeIdentity,
        relation: PackageReviewNominalIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDataProperties {
    pub multiplicity: language_core::Multiplicity,
}

impl PackageReviewDataProperties {}

// Stable public-domain evidence.
//
// `PackageReviewDomainSemanticRole` records closed compiler semantic roles
// without exporting compiler-private semantic-domain IDs across the canonical
// package-review boundary. `PackageReviewDomainEstablishmentRoute` records
// inert issuer identity without inventing traits for concrete machines or
// granting invocation/admission.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewDomainClassification {
    ProgressProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainSemanticRole {
    DenotationDimension,
    ArithmeticPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainEstablishmentKind {
    CheckedRequirement,
    BoundaryRequirement,
    ExactMachine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainEstablishmentRoute {
    CheckedRequirement {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    BoundaryRequirement {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    ExactMachine {
        machine_identity: PackageReviewNominalIdentity,
    },
}

impl PackageReviewDomainEstablishmentRoute {
    pub const fn kind(&self) -> PackageReviewDomainEstablishmentKind {
        match self {
            Self::CheckedRequirement { .. } => {
                PackageReviewDomainEstablishmentKind::CheckedRequirement
            }
            Self::BoundaryRequirement { .. } => {
                PackageReviewDomainEstablishmentKind::BoundaryRequirement
            }
            Self::ExactMachine { .. } => PackageReviewDomainEstablishmentKind::ExactMachine,
        }
    }
}

// Nominal declaration identity, toolchain source digests, and semantic
// dependency exposure vocabulary.
//
// `write_framed_identity` formats length-prefixed semantic identities
// (`len:label`) independent of source capture or persistence envelopes.
// `PackageReviewNominalOwner::Unresolved` surfaces any nominal reference that
// checked lowering retained without an authored source owner or mandatory
// compiler derivation origin so admission rejects it; `validate_resolved`
// enforces non-empty path + resolved owner across all record families.

pub fn write_framed_identity<'a>(
    writer: &mut impl std::fmt::Write,
    label: &str,
    children: impl IntoIterator<Item = &'a str>,
) -> std::fmt::Result {
    write!(writer, "{}:{label}", label.len())?;
    for child in children {
        write!(writer, "{}:{child}", child.len())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageReviewToolchainSourceIdentity {
    pub digest: [u8; 32],
}

impl PackageReviewToolchainSourceIdentity {
    field_projection_accessors!(valued digest: [u8; 32]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewNominalOwner {
    Package(PackageKeyIdentity),
    ToolchainSource(PackageReviewToolchainSourceIdentity),
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewNominalIdentity {
    pub owner: PackageReviewNominalOwner,
    pub path: String,
}

impl PackageReviewNominalIdentity {
    field_projection_accessors!(copied owner: PackageReviewNominalOwner);

    pub fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn validate_resolved(&self) -> Result<(), &'static str> {
        if self.path.is_empty() || self.owner == PackageReviewNominalOwner::Unresolved {
            Err("nominal identity is empty or unresolved")
        } else {
            Ok(())
        }
    }
}

/// One boundary effect. Its original declaration establishes ownership, while
/// an applied declaration also retains the compiler's normalized static inputs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewServiceIdentity {
    pub declaration: PackageReviewNominalIdentity,
    pub application: Option<self::PackageReviewTypeIdentity>,
}

impl From<PackageReviewNominalIdentity> for PackageReviewServiceIdentity {
    fn from(declaration: PackageReviewNominalIdentity) -> Self {
        Self {
            declaration,
            application: None,
        }
    }
}

impl PackageReviewServiceIdentity {
    pub fn owner(&self) -> PackageReviewNominalOwner {
        self.declaration.owner()
    }
    pub fn path(&self) -> &str {
        self.declaration.path()
    }
    pub(crate) fn validate_resolved(&self) -> Result<(), &'static str> {
        self.declaration.validate_resolved()?;
        if self.application.as_ref().is_some_and(|application| {
            !self
                .declaration
                .matches_applied_trait_declaration(&application.canonical)
        }) {
            return Err(
                "applied service identity does not name its declared receiver and complete arguments",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyExposure {
    PrivateImplementation,
    PublicInterface,
}

pub type PackageReviewSemanticDependencyKind = semantic_vocabulary::SemanticDependencyKind;

// Compiler-issued semantic-binding proposals paired with their exact checked
// service schemas (`SemanticBindingReviewCandidate`).
//
// The schema and any hosted permission rows are review proposals, not grants:
// permissions bind exact requirement identities to the compiler's hosted-leaf
// classifications, consumer scoping removes them from non-root/build inputs,
// and the resulting binding still requires complete checked recompilation and
// the project's explicit policy decision.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticBindingReviewCandidate {
    binding: AcceptedSemanticBinding,
    service_schema: ServiceSchema,
}

impl SemanticBindingReviewCandidate {
    pub fn new(binding: AcceptedSemanticBinding, service_schema: ServiceSchema) -> Self {
        debug_assert_eq!(
            binding.normalized_schema_digest(),
            service_schema.identity_digest(),
        );
        Self {
            binding,
            service_schema,
        }
    }

    field_projection_accessors!(borrowed binding: &AcceptedSemanticBinding);
    field_projection_accessors!(borrowed service_schema: &ServiceSchema);
}

// A declaration selected by source and a nominal carried through another API
// have different dependency contracts. A carried type commits to its supplier's
// whole checked source occurrence without granting source selection.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicySemanticDependencyConsumer {
    Callable(PackageReviewNominalIdentity),
    PackageImplementation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicySemanticDependency {
    Declaration {
        consumer: PackagePolicySemanticDependencyConsumer,
        dependency: PackageReviewNominalIdentity,
        exposure: PackageReviewSemanticDependencyExposure,
        kind: PackageReviewSemanticDependencyKind,
    },
    CarriedType {
        dependency: PackageReviewNominalIdentity,
        supplier_sources: PackageSourceConsumptionCommitment,
        exposure: PackageReviewSemanticDependencyExposure,
    },
}

impl PackagePolicySemanticDependency {
    pub fn dependency(&self) -> &PackageReviewNominalIdentity {
        match self {
            Self::Declaration { dependency, .. } | Self::CarriedType { dependency, .. } => {
                dependency
            }
        }
    }

    pub fn exposure(&self) -> PackageReviewSemanticDependencyExposure {
        match self {
            Self::Declaration { exposure, .. } | Self::CarriedType { exposure, .. } => *exposure,
        }
    }
}

// Authored deployment scope of a disclosed callable (`PackagePolicyTargetContext`).
//
// One activation reviews every authored leaf, including inactive private
// external supplies, so its selected target cannot stand in for each leaf's
// authored target. Uncatalogued tags remain valid inert disclosure for pure
// bodies, whereas a physical binding still requires a catalogued profile.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTargetContext {
    TargetIndependent,
    TargetMember { target: String },
}

impl PackagePolicyTargetContext {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if let Self::TargetMember { target } = self
            && target.is_empty()
        {
            return Err("authored target member has an empty target tag");
        }
        Ok(())
    }
}
