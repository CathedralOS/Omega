#![forbid(unsafe_code)]

//! The native compiler: retained Terminal plus realization inputs to an image.
//!
//! Start at `compiler.rs`. It prepares the supplied realization,
//! admits entry/providers and drives the abstract, target and physical stages.
//! Callback bodies take the same provider admission and physical pipeline,
//! using their own artifact-local native proposal and no application entry.
//! Source compilation, Build
//! execution and package review stay outside this dependency boundary; this
//! crate cannot obtain typed or checked trees to repair a missing Terminal fact.
//! The artifact owner retains runtime provisions before emission consumes them.
//! `application.rs` owns source-free Terminal application preparation and
//! package-permission admission, shared by artifact publication and native work.
//! It consumes exact source-issued inputs without acquiring or interpreting source.

mod application;
mod compiler;
mod components;
mod pipeline;
mod realization;
mod request;

#[cfg(test)]
mod pipeline_boundary;

pub use application::{TerminalApplication, admit_package_permissions};
pub use compiler::compile;
pub use components::ComponentCompilation;
pub use realization::RealizedNativeProduct;
pub use request::NativeCompileRequest;
pub use terminal_psi_to_abstract_operations::provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy,
};
pub use terminal_psi_to_abstract_operations::{ComponentExportInput, ComponentNativeInputs};
