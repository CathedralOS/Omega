#![forbid(unsafe_code)]

//! Program-storage entry contracts shared by source checking, calling-policy
//! planning, and native bridge construction.

mod root_role;
mod source_signature;

pub use root_role::ProgramStorageEntryRootRole;
pub use source_signature::{
    ProgramEntrySourceExtentFieldLayout, ProgramEntrySourceExtentFieldRole,
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramEntrySourceVisibleParameterSignature,
    SelectedProgramEntrySourceSignature,
};
