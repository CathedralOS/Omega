//! Optimizer module role: stage group. calls in abstract operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod placed_inputs;
pub use placed_inputs::AbstractOperationPlanWithPlacedViewInputs;
mod results;
pub use results::AbstractBoundaryResult;
mod dynamic;
pub use dynamic::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractParameterDynamicDispatch, AbstractReboundDynamicDispatch,
    AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch,
};
