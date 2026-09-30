#![forbid(unsafe_code)]

//! Joins a selected external-root provider plan to its installed
//! post-handoff entry writer.
//!
//! Start at `external_root_writer.rs`. Its free functions resolve one
//! external-root boundary slot from the provider selection retained on the
//! checked program, and route each entry claim to the exact checked
//! source-parameter fact its adapter consumes. The carriers then seal the
//! selected schema, the AOT-lowered writer, the installed resolver and an
//! activated unpublished destination before any source is resolved. They
//! carry that custody through binding, execution, consumer validation and
//! retry. Each rejection hands back the custody it was given instead of
//! reconstructing it.
//!
//! The mechanism lived in `provider-planning` behind an `installed-writer`
//! feature that no workspace crate enabled. It is its own crate because it
//! consumes installed code and external-root execution from the backend
//! runtime, which provider selection itself never needs. It reads provider
//! planning through three accessors: `exact_checked_adapter`,
//! `ProviderPlanningContext` and `same_semantic_name`.
//!
//! The joins grant no publication or provider-operation authority. Admitted
//! provider execution and the installed resolver supply that authority; a
//! selected-plan fingerprint here is identity only. No compiler route calls
//! this crate yet.

mod external_root_writer;

pub use external_root_writer::{
    AdmittedExternalRootEntryFactHandoff, BoundExternalRootPostHandoffWriterInvocation,
    BoundExternalRootWriterExecutionError, ExternalRootPostHandoffWriterBindingError,
    SelectedExternalRootEntryFactBinding, SelectedExternalRootPostHandoffWriterPreparation,
    SelectedExternalRootProviderPlan, SelectedExternalRootWriterPreparationError,
    ValidatedWrittenBoundExternalRootPostHandoffWriterDestination,
    WrittenBoundExternalRootConsumerValidationError,
    WrittenBoundExternalRootPostHandoffWriterDestination,
    WrittenBoundExternalRootWriterRecoveryError, bind_external_root_post_handoff_writer_invocation,
    optional_selected_external_root_provider_plan, selected_external_root_entry_fact_bindings,
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
};
