/*
Catalog provider and assumed-binding installation (`provider_installation`)
and artifact-bound target observation context
(`target_context`).
*/

pub(crate) mod provider_installation {
    pub use psi_provider_selections::{
        AdmittedAssumedBinding, AdmittedProviderSelections, AssumedBindingRejection,
        AssumedBindingSelection, ProviderInstallationError, ProviderInstallationSelection,
        admit_provider_execution_installation, admit_provider_installation,
        admit_provider_installation_from_artifact,
        admit_provider_installation_with_assumed_bindings_from_artifact,
    };
}

pub(crate) mod target_context {
    pub use psi_target_layouts::{
        TerminalEntryError, TerminalTargetContext, terminal_target_context,
    };
}
