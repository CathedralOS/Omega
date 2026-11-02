#![forbid(unsafe_code)]

//! Psi admission of source-level opaque representation relationships.
//!
//! Packages may publish ordinary named conformances, but only the exact
//! authoritative build machine can activate one as an opaque representation.
//! Given that caller-selected machine, Psi validates the source relationship
//! and retains its closed conformance
//! identity. Physical shape is derived by downstream target consumers from
//! the selected concrete carrier; source never supplies ABI numbers.
//!
//! Follow `representation_selection.rs` for build activation and final replay;
//! `representation_trait.rs` owns the shared compiler-role recognition check.

mod representation_selection;
mod representation_trait;

pub use representation_selections::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
    OpaqueRepresentationSelection, selected_application_commitment, selection_for_opaque,
};

pub use representation_selection::{
    FormedOpaqueRepresentation, collect_opaque_representation_selections,
    form_opaque_representation_request, rederive_opaque_representation_selections,
};
pub use representation_trait::is_compiler_owned_opaque_representation_trait;
