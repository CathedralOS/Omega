//! Function-relative realization after physical homes are known.
//!
//! This entrance owns the boundary's stage/scope vocabulary and exposes three
//! exact custody routes: selected lowering, x86 layout optimization, and
//! AArch64 CBNZ fusion. Manifest modeling/codec, owning carriers, shared
//! assembly, and route mechanics descend into their named leaves.

mod assembly;
mod carriers;
mod codec;
mod error;
mod model;
mod prelude;
mod routes;

pub use carriers::*;
pub use codec::FunctionRelativeOptimizationRealizationManifestDecodeError;
pub use error::FunctionRelativeOptimizationRealizationError;
pub use model::*;
pub use routes::*;

pub(crate) use assembly::{
    expected_aarch64_movn_manifest, function_relative_statistics, seal_function_relative_manifest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationStage {
    ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationScope {
    FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationUnavailableData {
    Unavailable,
}
