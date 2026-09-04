//! Source-closure corruption coverage.

use super::*;

#[test]
fn callee_machine_substitution_fails_closed() {
    let mut source = projected_structural_call_return_plan();
    let target = lower_to_target_operations(&source, NativeTarget::linux_x64()).unwrap();
    source.functions[1].machine = MachineId::new(999).unwrap();
    assert!(crate::validate_abstract_to_target_translation(
        &source,
        NativeTarget::linux_x64(),
        &target,
    )
    .is_err());
}
