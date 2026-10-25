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

mod compiler;
mod pipeline;
mod realization;
mod request;

#[cfg(test)]
mod pipeline_boundary;

pub use compiler::{compile, compile_with_components};
pub use realization::{
    ComponentExportImage, ComponentExportRequest, ComponentNativeCall, ComponentNativeExport,
    RealizedNativeProduct, emit_component_exports,
};
pub use request::NativeCompileRequest;
pub use terminal_psi_to_abstract_operations::{ComponentExportInput, ComponentNativeInputs};
