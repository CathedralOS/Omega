/*
Target-neutral boundary declarations, external provider contracts, runtime callable rows, dynamic
dispatch families, effect ceilings, and suspension/carry obligations in `TerminalModule`.
*/

mod application_demands;
pub use application_demands::TerminalBoundaryApplicationDemands;

pub(crate) mod applications;
pub(crate) mod conformances;
pub(crate) mod declarations;
pub(crate) mod external_providers;
pub use external_providers::{
    ExternalProviderConformance, ExternalProviderOwner, ExternalRequirementSchema,
};
pub(crate) mod non_observation;
pub(crate) mod numeric_intrinsics;
pub use numeric_intrinsics::validate_numeric_intrinsic_signature;
pub(crate) mod dynamic_dispatch;
pub(crate) mod providers;
pub(crate) mod reach_applications;
pub(crate) mod services;

pub use applications::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationCandidate,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, TerminalBoundaryApplicationDemand,
};
pub use conformances::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedConformanceStaticApplication,
    DeclaredResultLifetime, closed_conformance_application_commitment,
    closed_conformance_application_report_fingerprint,
};
pub use declarations::{
    BoundaryContentGuarantee, BoundaryDeclarationRole, BoundaryLifetimeDependency,
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryReferenceDestination, BoundaryReferenceOutput, BoundaryStructuralResultDeclaration,
    CheckedBoundaryCallable, CheckedBoundaryExecution, ContentConservationGuarantee,
    ProgramLocalRootIntroductionSchema, ScalarDomainRequirement, StructuralDomainRequirement,
    find_boundary_machine, program_local_root_introduction_compatibility_report_identity,
};
pub use dynamic_dispatch::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceOrigin, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable,
};
pub use providers::{
    ProviderCandidateConformance, ProviderInstanceRequirements, ProviderParameterRefinement,
    ProviderRefinement, ProviderSignature, ProviderSignatureParameter, RequiredProviderInstance,
};
pub use reach_applications::{
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachCapture, ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
    closed_reach_template_commitment,
};
pub use services::{InstallationReachDependency, ServiceDeclaration, TerminalRootServiceReach};

pub(crate) mod result_regions;
pub use result_regions::ReferenceResultShape;
