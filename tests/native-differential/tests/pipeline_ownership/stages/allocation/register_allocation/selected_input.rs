use crate::tests::*;
#[test]
fn forwarded_parameter_selection_rejects_fixed_input_and_path_corruption() {
    let staged = staged_forwarded_conditional(NativeTarget::linux_x64());
    let mut corrupted = staged.selected().plan().clone();
    corrupted.functions[0].virtual_registers[1].entry_fixed_view = None;
    assert!(matches!(
        validate_raw_selection(&staged, corrupted),
        Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
    ));

    let mut corrupted = staged.selected().plan().clone();
    let SelectedTerminator::Return { instruction, .. } =
        &mut corrupted.functions[0].blocks[1].terminator
    else {
        unreachable!()
    };
    instruction.operands[0].virtual_register = VirtualRegisterId(0);
    assert!(matches!(
        validate_raw_selection(&staged, corrupted),
        Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
    ));
}
