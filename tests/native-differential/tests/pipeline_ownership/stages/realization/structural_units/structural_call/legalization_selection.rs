use crate::tests::*;

pub(super) fn lower_and_select_structural_call() -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = structural_extent_call_unit_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, NativeTarget::uefi_x64()).unwrap();
    let legalized = legalize_target_operations(
        target.target_operations(),
        target.optimized().plan(),
        target.optimized().unit(),
    )
    .expect("structural Unit call must reach the distinct v6 custody roster");

    assert!(legalized.plan().unit_functions.is_empty());
    assert_eq!(legalized.plan().structural_unit_functions.len(), 2);
    assert_eq!(legalized.receipt().function_count(), 2);
    let caller = &legalized.plan().structural_unit_functions[0];
    let callee = &legalized.plan().structural_unit_functions[1];
    assert_eq!(caller.parameters.len(), 2);
    assert_eq!(callee.parameters.len(), 2);
    assert!(callee.call.is_none());
    let call = caller.call.as_ref().expect("caller retains one Unit call");
    assert_eq!(call.arguments.len(), 2);
    assert!(call.claim_transfers.is_empty());
    assert!(matches!(
        call.ownership.as_slice(),
        [OwnershipEvent::ClaimTransfer(claims)] if claims.is_empty()
    ));
    assert!(matches!(
        caller.return_ownership.as_slice(),
        [OwnershipEvent::Cleanup(cleanups)] if cleanups.is_empty()
    ));
    assert_eq!(call.effect.output, caller.return_effect.input);
    for (parameter, register, copy_offset) in [
        (&caller.parameters[0], MachineRegister::X86Rcx, 32),
        (&caller.parameters[1], MachineRegister::X86Rdx, 48),
    ] {
        assert!(matches!(
            parameter.target.placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(actual),
                copy_stack_byte_offset: Some(actual_offset),
                byte_size: 16,
                alignment: 8,
            }] if *actual == register && *actual_offset == copy_offset
        ));
    }
    assert_eq!(
        call.arguments[0].target.source,
        caller.parameters[0].target.placement
    );
    assert_eq!(
        call.arguments[1].target.source,
        caller.parameters[1].target.placement
    );
    assert_eq!(
        call.arguments[0].target.destination,
        callee.parameters[0].target.placement
    );
    assert_eq!(
        call.arguments[1].target.destination,
        callee.parameters[1].target.placement
    );

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0].parameters.swap(0, 1);
    assert!(
        validate_legalized_operations(
            target.target_operations(),
            target.optimized().plan(),
            target.optimized().unit(),
            corrupted,
        )
        .is_err()
    );
    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .effect
        .output += 1;
    assert!(
        validate_legalized_operations(
            target.target_operations(),
            target.optimized().plan(),
            target.optimized().unit(),
            corrupted,
        )
        .is_err()
    );

    let selected = stage_optimized_instruction_selection(target)
        .expect("the exact Microsoft-x64 structural call must reach selected v9");
    assert_eq!(selected.custody().function_count(), 2);
    assert!(selected.selected().plan().functions.is_empty());
    assert_eq!(
        selected.selected().plan().structural_unit_functions.len(),
        2
    );
    let selected_caller = &selected.selected().plan().structural_unit_functions[0];
    let selected_call = selected_caller
        .call
        .as_ref()
        .expect("caller owns one atomic structural Unit call");
    assert_eq!(selected_call.id, SelectedInstructionId(0));
    assert_eq!(
        selected_call.constraint,
        omega_isa_x86_64::X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR
    );
    assert!(selected_call.implicit_uses.len() >= 4);
    assert!(selected_call.implicit_defs.len() >= 2);
    assert!(!selected_call.clobbers.is_empty());
    assert_eq!(
        selected_caller.terminator.instruction.id,
        SelectedInstructionId(1)
    );

    selected
}
