//! Optimizer module role: stage group. Function-relative realization after physical homes are known.
//!
//! This entrance owns the boundary vocabulary and the three route families:
//! baseline selected lowering, function-relative layout, and the single
//! rule-independent post-allocation machine join.  Carriers, manifests,
//! assembly, codecs, and route mechanics descend into named leaves below.

mod assembly;
mod unit;
pub use unit::*;
mod carriers;
#[cfg(any(test, feature = "test-support"))]
mod fixed_frame_test_support;
mod frame;
#[cfg(any(test, feature = "test-support"))]
pub use fixed_frame_test_support::*;
pub use frame::FunctionRelativeFrame;
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

pub(crate) use assembly::{function_relative_statistics, seal_function_relative_manifest};

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
