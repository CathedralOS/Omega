//! Bodyless requirements, service reach, and realization candidates.

mod conformances;
mod declarations;
mod dynamic_dispatch;
mod providers;
mod reach_applications;
mod services;

pub use conformances::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRealizationCallable, ClosedConformanceRow,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};
pub use declarations::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryParameterKind, BoundaryStructuralResultDeclaration, ContentConservationGuarantee,
    ProgramLocalRootIntroductionSchema, StructuralDomainRequirement,
    program_local_root_introduction_compatibility_report_identity,
};
pub use dynamic_dispatch::{
    TerminalDirectDynamicDispatch, TerminalDynamicConformanceSelection,
    TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorParameter,
    TerminalDynamicDescriptorSource, TerminalDynamicDispatchCatalog, TerminalDynamicRequirement,
    TerminalIndirectDynamicDispatch, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, TerminalStoredDynamicDescriptor,
    TerminalStoredDynamicDispatch,
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
