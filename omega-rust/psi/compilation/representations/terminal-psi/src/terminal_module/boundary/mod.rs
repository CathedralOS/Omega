//! Bodyless requirements, service reach, and realization candidates.

mod applications;
mod conformances;
mod declarations;
mod dynamic_dispatch;
mod providers;
mod reach_applications;
mod services;

pub use applications::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationCandidate,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, TerminalBoundaryApplicationDemand,
};
pub use conformances::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedConformanceStaticApplication,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};
pub use declarations::{
    BoundaryContentGuarantee, BoundaryDeclarationRole, BoundaryLifetimeDependency,
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryReferenceDestination, BoundaryReferenceOutput, BoundaryStructuralResultDeclaration,
    CheckedBoundaryCallable, ContentConservationGuarantee, ProgramLocalRootIntroductionSchema,
    StructuralDomainRequirement, find_boundary_machine,
    program_local_root_introduction_compatibility_report_identity,
};
pub use dynamic_dispatch::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicReferenceOrigin, TerminalDynamicReferenceRegion, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable,
};
pub use providers::{
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter,
};
pub use reach_applications::{
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
};
pub use services::{InstallationReachDependency, ServiceDeclaration, TerminalRootServiceReach};
