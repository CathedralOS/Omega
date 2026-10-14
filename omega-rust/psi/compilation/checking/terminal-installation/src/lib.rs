#![forbid(unsafe_code)]

//! Source-free receiving-boundary contracts shared by native realization and
//! interpretation. Provider policy arrives beside Terminal Psi, never inside it.
//!
//! `provider_installation` admits exact catalog selections and explicit assumed
//! links; `entry_values` describes externally established structural backing.
//! `target_families` selects exact tagged members without proving refinement or
//! granting execution authority. Consumers still admit the portable contract
//! and transport the selected member's authentic result evidence.
//! This owner has no execution loop, host adapters, mutable guest storage, or
//! native ABI policy. Authored layout closure retains a separate exact overlay.
//! Each consumer establishes its own invocation from these facts.

mod entry_values;
mod private_entry_ram;
mod provider_installation;
mod target_families;

pub use entry_values::{TerminalPlacedViewEstablishment, TerminalStructuralValue};
pub use private_entry_ram::{
    CheckedPrivateEntryRamRecipe, PrivateEntryRamInitialization, PrivateEntryRamRecipe,
    PrivateEntryRamScalarInitialization, admit_private_entry_ram, private_entry_ram_identity_pair,
};
pub use provider_installation::{
    AdmittedAssumedBinding, AdmittedProviderSelections, AssumedBindingRejection,
    AssumedBindingSelection, ProviderInstallationError, ProviderInstallationSelection,
    admit_provider_installation, admit_provider_installation_from_artifact,
    admit_provider_installation_with_assumed_bindings_from_artifact,
};
pub use target_families::{
    AdmittedTargetMachineSelections, TargetMachineSelection, TargetMachineSelectionError,
    admit_target_machine_selections, resolve_target_machine_entry, resolve_target_machine_member,
};

mod target_layouts;
pub use target_layouts::{
    AdmittedTargetLayouts, TargetLayoutClosure, TargetLayoutGeometry, TargetLayoutSelectionError,
    admit_target_layouts,
};
