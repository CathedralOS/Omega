use crate::tests::*;

pub(super) fn verify_structural_call_encoding_and_layout(homes: StagedOptimizedRegisterHomes) {
    let legality_stage = homes.legality_stage();
    let range_stage = legality_stage.live_range_stage();
    let environment = range_stage
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    assert_eq!(post.machine().plan().functions.len(), 2);
    let call = post.machine().plan().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| {
            row.alternative.key.family == selected_instructions::MachineAlternativeFamily::CallUnit
        })
        .unwrap();
    let call_identity = call.instruction;
    assert!(!call.unit_clobbers.is_empty());
    let mut corrupted = post.machine().plan().clone();
    corrupted.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.instruction == call_identity)
        .unwrap()
        .unit_clobbers
        .clear();
    corrupted.identity = physical_instructions::post_allocation_machine_identity(&corrupted);
    let corrupted =
        physical_instructions::PostAllocationMachinePlan::decode(&corrupted.encode()).unwrap();
    assert!(
        register_homes_to_post_allocation_machine::validate_post_allocation_machine_plan(
            range_stage.liveness_stage().selected_stage().selected(),
            post.effects(),
            range_stage.ranges(),
            legality_stage.legality(),
            homes.homes(),
            homes.post_allocation_manifest(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            corrupted,
        )
        .is_err()
    );

    let mut realization = stage_fixed_frame_function_relative_realization(
        homes.try_into().unwrap(),
        post,
        selected_lowering_budget(),
    )
    .unwrap();
    let encoding = realization.encoding();
    assert!(encoding.rows().iter().any(|row| matches!(
        row.state,
        SelectedFormEncodingState::UnresolvedInternalMachineCall { .. }
    )));
    let original_encoding = encoding.clone();
    let original_layout = realization.baseline_layout().clone();
    assert_eq!(
        original_encoding.program().frame.as_ref(),
        Some(realization.frame().plan())
    );
    for mutation in 0..5 {
        let program = realization.encoding_mut().program_mut_for_test();
        match mutation {
            0 => program.frame = None,
            1 => {
                let frame = program
                    .frame
                    .as_mut()
                    .unwrap()
                    .functions
                    .iter_mut()
                    .find(|frame| frame.contains_call)
                    .unwrap();
                frame.outgoing_abi_area.byte_size -= 8;
            }
            _ => {
                let address = program
                    .rows
                    .iter_mut()
                    .filter_map(|row| row.address.as_mut())
                    .find(|address| {
                        matches!(
                            address.symbolic,
                            physical_instructions::PhysicalAddressOperation::Store64 { .. }
                        )
                    })
                    .unwrap();
                let physical_instructions::PhysicalAddressOperation::Store64 { slot, byte_offset } =
                    address.symbolic
                else {
                    unreachable!();
                };
                match mutation {
                    2 => address.displacement += 8,
                    3 => {
                        address.symbolic =
                            physical_instructions::PhysicalAddressOperation::FrameAddress {
                                slot,
                                byte_offset,
                            }
                    }
                    _ => {
                        address.symbolic =
                            physical_instructions::PhysicalAddressOperation::Store64 {
                                slot,
                                byte_offset: byte_offset + 8,
                            }
                    }
                }
            }
        }
        program.identity = program.recomputed_identity();
        assert!(
            validate_optimized_layout_independent_selected_form_encoding(
                realization.allocation().current().selected(),
                realization.machine(),
                realization
                    .allocation()
                    .current()
                    .register_environment()
                    .physical(),
                Some(realization.frame().plan()),
                realization.encoding(),
            )
            .is_err(),
            "reauthenticated frame/address mutation {mutation} must fail encoding replay"
        );
        assert!(validate_fixed_frame_function_relative_realization(&realization).is_err());
        *realization.encoding_mut() = original_encoding.clone();
        validate_fixed_frame_function_relative_realization(&realization).unwrap();
    }
    for mutation in 0..4 {
        let row = realization
            .encoding_mut()
            .rows_mut()
            .iter_mut()
            .find(|row| {
                matches!(
                    row.state,
                    SelectedFormEncodingState::UnresolvedInternalMachineCall { .. }
                )
            })
            .unwrap();
        let SelectedFormEncodingState::UnresolvedInternalMachineCall {
            bytes,
            footprint,
            fixup,
        } = &mut row.state
        else {
            unreachable!()
        };
        match mutation {
            0 => bytes[0] ^= 1,
            1 => fixup.callee = MachineId::new(999_999).unwrap(),
            2 => footprint.implicit_clobbers.clear(),
            _ => row.instruction = SelectedInstructionId(u32::MAX),
        }
        assert!(
            validate_fixed_frame_function_relative_realization(&realization).is_err(),
            "encoding mutation {mutation}"
        );
        *realization.encoding_mut() = original_encoding.clone();
        validate_fixed_frame_function_relative_realization(&realization).unwrap();
    }
    for mutation in 0..3 {
        let layout = realization.baseline_layout_mut().functions_mut();
        match mutation {
            0 => layout.reverse(),
            1 => layout[0].blocks[0].instructions[0].offset += 1,
            _ => layout[0].blocks[0].instructions[0].bytes[0] ^= 1,
        }
        assert!(
            validate_fixed_frame_function_relative_realization(&realization).is_err(),
            "layout mutation {mutation}"
        );
        *realization.baseline_layout_mut() = original_layout.clone();
    }
    validate_fixed_frame_function_relative_realization(&realization).unwrap();
    let current = realization.encoding().shared_program();
    let identity = current.identity;
    drop(realization);
    assert_eq!(current.recomputed_identity(), identity);
}
