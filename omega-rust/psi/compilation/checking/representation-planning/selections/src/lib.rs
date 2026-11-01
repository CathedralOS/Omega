#![forbid(unsafe_code)]

//! Durable identities for compiler-validated opaque representation selections.
//!
//! Build planning owns validation and construction. Lower layers consume this
//! target-independent record without depending on the build-planning service.
//! Start at [`representation_selections::OpaqueRepresentationSelection`].

pub mod representation_selections;
pub use representation_selections::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
    OpaqueRepresentationSelection, selected_application_commitment, selection_for_opaque,
};
