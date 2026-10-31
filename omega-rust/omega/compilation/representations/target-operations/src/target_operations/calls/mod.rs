//! Optimizer module role: stage group. calls in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod inputs;
pub use inputs::{
    TargetAccessViewInput, TargetNativeCallbackArgument, TargetOperationPlanWithAccessViewInputs,
};
mod abi;
pub use abi::{MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi};
mod arguments;
pub use arguments::{
    TargetBoundaryResult, TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument,
};
mod results;
pub use results::{TargetCallResult, TargetStructuralResultContract};
mod targets;
pub use targets::{NativeCallTarget, selector_carrier_type};

mod access_bindings;
pub use access_bindings::{
    TargetAccessCallAssociation, TargetAccessCallBinding, TargetAccessInterpretation,
};
