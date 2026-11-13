/*
Provider admission for native realization, and the installation custody that
`provider_installation` joins to the admitted artifact's retained module.

We keep all sealed admission evidence here: boundary settlements and their
executions, admitted native callback arguments, and the admitted provider
installation. Their constructors stay private to this package so only our
admission entrypoints can mint them.
*/

/*
Terminal-to-abstract lowering (Omega 00) runs this admission, and later stages
and backends read the sealed evidence by type without depending on that stage.
This package is standalone admission evidence rather than a pipeline stage.
*/

#![forbid(unsafe_code)]

pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};

pub mod provider_admission;
pub mod provider_installation;

pub(crate) use optimization_core::digest_encoder;

pub use provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedCallbackEntry,
    AdmittedFatalInterruptEntry, AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptHelper,
    AdmittedInstalledInterruptScope, AdmittedInterruptContextAnchor,
    AdmittedNativeCallbackArgument, InstalledInterruptNativeEntryProposal,
    NativeCallbackArgumentProposal, SettledCompilerBuiltin,
};
pub use provider_installation::{
    AdmittedProviderInstallation, ProviderInstallationError, SelectedProviderAdapter,
};

mod admission_source;
mod lowering_facts;
pub use admission_source::{
    AdmissionSource, AdmittedInstalledEntryExecution, AdmittedProgramEntryCall, ComponentCallFacts,
    selected_calling_boundary,
};
pub use lowering_facts::{AdmittedInstalledInterruptCohort, AdmittedNativeCallbackCohort};
