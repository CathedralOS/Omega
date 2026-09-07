//! Optimizer module role: stage group. Function-relative realization after physical homes are known.
//!
//! One canonical framed realization retains allocation and layout replay evidence.

mod assembly;
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
