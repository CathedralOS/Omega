/*
We validate source-free receiving-boundary installation contracts over Terminal
Psi artifacts (`admit_provider_installation`, `admit_component_provider`,
`admit_target_layouts`,
`admit_private_entry_ram`, and `admit_compiler_evaluation`).
*/

/*
Our boundary-admission checks operate on canonical Terminal Psi without
reopening source trees or embedding host execution loops: provider policy,
target layouts, scalar application closures,
and synchronous call graphs are admitted alongside the artifact so both the
Terminal Psi interpreter and Omega native realization enforce identical
installation prerequisites.
*/

/*
`terminal-installation` consumes `terminal-psi` and `layout-plans` and serves
the Psi interpreter, compile-time evaluation, and Omega native realization.
*/

#![forbid(unsafe_code)]

pub mod declaration_selection;
mod evaluation_and_synchronous_calls;
mod evaluation_requirements;
pub mod logical_reach;
mod provider_and_target_installation;

pub mod selection_questions;

pub use psi_authorization::{
    ArtifactTrustError, AuthorizedTerminalArtifact, HostProducerTrust, authorize_terminal_artifact,
};

pub(crate) use evaluation_and_synchronous_calls::compiler_evaluation;
pub(crate) use provider_and_target_installation::{
    component_memberships, component_provider, provider_installation, target_context,
};

pub(crate) mod entry_values {
    pub use terminal_semantics::entry_values::{
        TerminalAccessViewEstablishment, TerminalScalarValue, TerminalStructuralValue,
    };
}

pub(crate) mod private_entry_ram {
    pub use psi_entry_memory::{
        CheckedPrivateEntryRamRecipe, PrivateEntryRamInitialization, PrivateEntryRamRecipe,
        PrivateEntryRamScalarInitialization, admit_private_entry_ram,
        decode_private_entry_observations, private_entry_ram_identity_pair,
    };
}

pub(crate) mod target_layouts {
    pub use psi_target_layouts::{
        AdmittedTargetLayouts, TargetLayoutClosure, TargetLayoutGeometry,
        TargetLayoutSelectionError, admit_target_layouts,
    };
}

pub use compiler_evaluation::{
    AdmittedCompilerEvaluation, InvocationSourceSelection, admit_compiler_evaluation,
    admit_compiler_evaluation_with_source_consent, admit_declaration_selections,
};
pub use component_provider::{AdmittedComponentProvider, admit_component_provider};
pub use entry_values::{
    TerminalAccessViewEstablishment, TerminalScalarValue, TerminalStructuralValue,
};
pub use layout_plans::LayoutTargetFacts;
pub use private_entry_ram::{
    CheckedPrivateEntryRamRecipe, PrivateEntryRamInitialization, PrivateEntryRamRecipe,
    PrivateEntryRamScalarInitialization, admit_private_entry_ram,
    decode_private_entry_observations, private_entry_ram_identity_pair,
};
pub use provider_installation::{
    AdmittedAssumedBinding, AdmittedProviderSelections, AssumedBindingRejection,
    AssumedBindingSelection, ProviderInstallationError, ProviderInstallationSelection,
    admit_provider_execution_installation, admit_provider_installation,
    admit_provider_installation_from_artifact,
    admit_provider_installation_with_assumed_bindings_from_artifact,
};
pub use semantic_vocabulary::ScalarAddressModel;
pub use psi_scalar_applications::{
    AdmittedScalarApplications, AdmittedScalarEntry, CallingAbiStorageDemand,
    ScalarApplicationClosure, ScalarApplicationObservations, ScalarApplicationValues,
    ScalarConditionSelection, ScalarEntryChecks, ScalarEntryPrerequisites,
    ScalarLayoutObservationError, ScalarScopeInputs,
};
pub use target_context::{TerminalEntryError, TerminalTargetContext, terminal_target_context};
pub use target_layouts::{
    AdmittedTargetLayouts, TargetLayoutClosure, TargetLayoutGeometry, TargetLayoutSelectionError,
    admit_target_layouts,
};

pub mod application;
