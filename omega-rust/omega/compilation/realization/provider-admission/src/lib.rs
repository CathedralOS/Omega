#![forbid(unsafe_code)]

//! Provider admission for native realization, and the installation custody
//! `provider_installation` joins to the admitted artifact's retained module.
//!
//! The sealed admission evidence lives here: boundary settlements and their
//! executions, admitted native callback arguments, and the admitted provider
//! installation. Their constructors stay private to this package, so only the
//! admission below can mint them. Terminal-to-abstract lowering (Omega 00)
//! runs this admission, and the later stages and backends read the evidence
//! by type without depending on that stage. This is standalone evidence, not
//! a pipeline stage.

pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};

mod installed_entry_execution;
pub use installed_entry_execution::AdmittedInstalledEntryExecution;

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
    AdmissionSource, AdmittedProgramEntryCall, ComponentCallFacts, selected_calling_boundary,
};
pub use lowering_facts::{
    AdmittedInstalledInterruptCohort, AdmittedLoweringFacts, AdmittedNativeCallbackCohort,
    LoweringEntryCall,
};
