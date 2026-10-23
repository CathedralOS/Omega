#![forbid(unsafe_code)]
//! Abstract operations to target operations.
//!
//! Production enters through `lower_optimized_to_target_operations`
//! (`lowering/optimized.rs`). It takes a validated optimized abstract plan and
//! its admitted settlements, binds the admitted provider executions, lowers the
//! plan through `lowering/coordination.rs`, and retains the lowered program
//! beside those inputs. Provider installation identity is joined while the
//! program is produced; translation replay belongs to the independent audit,
//! not this producer's default route. Stage 00 owns the admitted settlement
//! facts. The `placed_view_inputs` route is the in-progress direct-entry
//! placed-view slice (TASKS.md PLAN-LAID-VIEWS).
//!
//! Operation dispatch does not preflight borrow legality, and calls transport
//! the existing descriptor leaves of owned operands rather than reconstructing
//! their permissions from types. Storage owners still join current homes,
//! ordered pointer crossings and ABI placements; the stage 03 corpus audit
//! independently exercises custody. Returns transport the current value home
//! without replaying returned-loan admission. Formation, copying, release and
//! control-flow joins carry descriptor types and possible backing origins,
//! without parent-loan graphs or formation histories. Source permission belongs
//! to Psi; physical type, layout and ABI constraints remain native-owned.

mod lowering;
mod structural_shapes;

pub use lowering::descriptor_input_guard::{
    DescriptorInputGuard, descriptor_input_guard, descriptor_input_guard_for_call,
};
pub use lowering::model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    LoweringError,
};
pub use lowering::optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub use lowering::placed_view_inputs;

// Receiving stages reconstruct ABI classes from their own declaration inputs,
// using the same boundary classification rather than a second type roster.
pub use structural_shapes::classified_boundary_shape;
