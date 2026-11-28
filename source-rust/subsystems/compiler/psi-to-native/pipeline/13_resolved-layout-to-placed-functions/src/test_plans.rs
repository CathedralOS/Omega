//! Placeholder frame and layout plans for unit tests that need a plan to point
//! at but never read its rows. Both point at the same placeholder machine plan.
use std::sync::Arc;

use machine_code::{ResolvedMachineLayout, TargetFrameLayoutPlan};
use target::NativeTarget;

pub(crate) fn frame_plan() -> Arc<TargetFrameLayoutPlan> {
    Arc::new(machine_code::test_support::placeholder_frame_plan(
        Vec::new(),
    ))
}

pub(crate) fn resolved_layout() -> Arc<ResolvedMachineLayout> {
    Arc::new(ResolvedMachineLayout {
        machine: post_allocation_machine::test_support::placeholder_plan(),
        pre_layout: machine_code::test_support::placeholder_encoding(),
        target: NativeTarget::linux_x64(),
        policy: machine_code::SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        functions: Vec::new(),
    })
}
