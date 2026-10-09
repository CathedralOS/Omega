#![forbid(unsafe_code)]

//! Source-free receiving-boundary contracts shared by native realization and
//! interpretation. Provider policy arrives beside Terminal Psi, never inside it.
//!
//! `provider_installation` admits exact catalog selections and explicit assumed
//! links; `entry_values` describes externally established structural backing.
//! This owner has no execution loop, host adapters, mutable guest storage, or
//! target layout. Each consumer establishes its own invocation from these facts.

mod entry_values;
mod provider_installation;

pub use entry_values::{TerminalPlacedViewEstablishment, TerminalStructuralValue};
pub use provider_installation::{
    AdmittedAssumedBinding, AdmittedProviderSelections, AssumedBindingRejection,
    AssumedBindingSelection, ProviderInstallationError, ProviderInstallationSelection,
    admit_provider_installation, admit_provider_installation_from_artifact,
    admit_provider_installation_with_assumed_bindings_from_artifact,
};
