use crate::tests::*;
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarInstructionKind, LegalizedScalarTerminator,
};

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
    .expect("owned structural arguments use ordinary ordered call instructions");
    assert_eq!(legalized.plan().scalar_functions.len(), 2);
    assert_eq!(legalized.receipt().function_count(), 2);
    let caller = &legalized.plan().scalar_functions[0];
    let callee = &legalized.plan().scalar_functions[1];
    let parameters = &caller.structural.as_ref().unwrap().parameters;
    let callee_parameters = &callee.structural.as_ref().unwrap().parameters;
    assert_eq!(parameters.len(), 2);
    assert_eq!(callee_parameters.len(), 2);
    assert!(callee.blocks[0].instructions.is_empty());
    let instruction = &caller.blocks[0].instructions[0];
    assert!(instruction.result.is_none());
    let LegalizedScalarInstructionKind::Call(call) = &instruction.kind else {
        panic!("ordinary call");
    };
    assert_eq!(call.arguments.len(), 2);
    assert!(call.result_placement.is_none());
    assert!(call.claim_transfers.is_empty());
    assert!(
        matches!(instruction.ownership.as_slice(), [OwnershipEvent::ClaimTransfer(claims)] if claims.is_empty())
    );
    let LegalizedScalarTerminator::Return(returned) = &caller.blocks[0].terminator else {
        panic!("Unit return");
    };
    assert!(
        matches!(returned.ownership.as_slice(), [OwnershipEvent::Cleanup(cleanups)] if cleanups.is_empty())
    );
    assert_eq!(instruction.effect.output, returned.effect.input);
    for (parameter_index, register, copy_offset) in [
        (0, MachineRegister::X86Rcx, 32),
        (1, MachineRegister::X86Rdx, 48),
    ] {
        let parameter = &parameters[parameter_index];
        assert!(matches!(parameter.target.placement.locations.as_slice(),
            [ValueLocation::Indirect { pointer: IndirectPointerLocation::Register(actual),
                copy_stack_byte_offset: Some(actual_offset), byte_size:16, alignment:8 }]
            if *actual == register && *actual_offset == copy_offset));
        let LegalizedScalarArgument::Structural {
            semantic,
            target: argument,
        } = &call.arguments[parameter_index]
        else {
            panic!("structural ownership argument");
        };
        assert_eq!(semantic.place, parameter.semantic.place);
        assert_eq!(semantic.access, StructuralAccess::Owned);
        assert_eq!(argument.source, parameter.target.placement);
        assert_eq!(
            argument.destination,
            callee_parameters[parameter_index].target.placement
        );
    }
    for mutation in 0..3 {
        let mut corrupted = legalized.plan().clone();
        match mutation {
            0 => corrupted.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters
                .swap(0, 1),
            1 => {
                corrupted.scalar_functions[0].blocks[0].instructions[0]
                    .effect
                    .output += 1
            }
            _ => {
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut corrupted.scalar_functions[0].blocks[0].instructions[0].kind
                else {
                    unreachable!()
                };
                call.arguments.swap(0, 1);
            }
        }
        assert!(
            validate_legalized_operations(
                target.target_operations(),
                target.optimized().plan(),
                target.optimized().unit(),
                corrupted
            )
            .is_err()
        );
    }
    let selected = stage_optimized_instruction_selection(target).unwrap();
    assert_eq!(selected.custody().function_count(), 2);
    assert_eq!(selected.selected().plan().functions.len(), 2);
    let caller = &selected.selected().plan().functions[0];
    assert_eq!(caller.calls.len(), 1);
    assert_eq!(caller.outgoing_arguments.len(), 2);
    assert!(!caller.virtual_registers.is_empty());
    let call = &caller.calls[0];
    let instruction = caller
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| row.id == call.instruction)
        .unwrap();
    assert!(
        matches!(instruction.kind, SelectedInstructionKind::CallUnit {callee} if callee == call.call.callee)
    );
    assert_eq!(call.call.arguments.len(), 2);
    let original = selected.selected().plan().clone();
    for mutation in 0..3 {
        let mut changed = original.clone();
        let caller = &mut changed.functions[0];
        match mutation {
            0 => caller.calls[0].call.arguments.swap(0, 1),
            1 => caller.outgoing_arguments[0].abi_stack_byte_offset += 8,
            _ => caller.memory_accesses[0].byte_offset += 8,
        }
        assert!(validate_raw_selection(&selected, changed).is_err());
    }
    selected
}
