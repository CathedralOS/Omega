//! Lower optimized abstract operations, prepare their target instructions and publish the facts needed by emission.
#![forbid(unsafe_code)]

pub use native_body_plan::NativeBodyPlan;

mod abstract_input;
mod descriptor_input;
mod emission_facts;
mod preparation;
mod target_lowering;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub use abstract_input::{AbstractInputError, admit_abstract_body};
pub use target_lowering::{LoweringError, OptimizedTargetLoweringRequest, lower_native_body};
pub mod replay {
    pub use crate::preparation::{
        TargetOperationAuditError, verify_execution_plan,
        verify_target_operation_plan_with_closed_module,
    };
    pub use crate::target_lowering::require_lowering_context;
}
