#![forbid(unsafe_code)]

//! Data-only declarations that join a selected source entry to its target
//! contract and native realization. This crate owns no emitted bytes,
//! installation state, or legacy backend pipeline.

mod boundary_entry_storage;
mod diagnostic;
mod optimized_semantic_entry;
mod optimized_semantic_wrapper;
mod post_handoff_writer;
mod program_entry_physical;
mod root_role;
mod selected_entry;
mod source_signature;

pub use boundary_entry_storage::{
    DerivedBoundaryEntryParameterStorage, DerivedBoundaryEntryStorage,
};
pub use diagnostic::ProgramStorageEntryDiagnostic;
pub use optimized_semantic_entry::*;
pub use optimized_semantic_wrapper::*;
pub use post_handoff_writer::*;
pub use program_entry_physical::*;
pub use root_role::ProgramStorageEntryRootRole;
pub use selected_entry::SelectedProgramStorageEntryPlan;
pub use source_signature::{
    ProgramEntrySourceExtentFieldLayout, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramEntrySourceSignatureIdentity,
    ProgramEntrySourceVisibleParameterSignature, SelectedProgramEntrySourceSignature,
};
