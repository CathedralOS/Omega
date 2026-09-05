//! Optimizer module role: stage group. Exact legalization custody and fences.

use crate::tests::fixtures::projected_structural_call_return::projected_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};
use target::NativeTarget;

mod corruption;
mod fence;
mod positive;

fn targets() -> [NativeTarget; 5] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ]
}
