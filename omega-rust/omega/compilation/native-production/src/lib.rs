#![forbid(unsafe_code)]

//! The native compiler: retained Terminal plus realization inputs to an image.
//!
//! Start at `native_compilation.rs`. It prepares the supplied realization,
//! admits entry/providers and drives the abstract, target and physical stages.
//! Callback bodies take the same provider admission and physical pipeline,
//! using their own artifact-local native proposal and no application entry.
//! Source compilation, Build
//! execution and package review stay outside this dependency boundary; this
//! crate cannot obtain typed or checked trees to repair a missing Terminal fact.
//! The artifact owner retains runtime provisions before emission consumes them.

mod component_inputs;
mod native_compilation;
mod physical_pipeline;
mod realization;
mod request;

pub use component_inputs::{ComponentExportInput, ComponentNativeInputs};
pub use native_compilation::compile;
pub use realization::{RealizedNativeProduct, ComponentExportImage, ComponentExportRequest, ComponentNativeExport, emit_component_exports};
pub use request::NativeCompileRequest;
