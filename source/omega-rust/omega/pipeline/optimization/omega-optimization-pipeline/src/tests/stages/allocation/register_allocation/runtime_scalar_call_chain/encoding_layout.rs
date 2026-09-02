use crate::tests::*;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_target::Architecture;

use super::fixture::{caller_machine, staged_homes};

fn staged_call_encoding(
    target: NativeTarget,
) -> (
    StagedOptimizedRegisterHomes,
    StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding,
) {
    let homes = staged_homes(target);
    let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        &post,
        selected_stage.register_environment().physical(),
    )
    .unwrap();
    (homes, post, encoding)
}

#[test]
fn target_owned_unresolved_call_templates_survive_layout_on_both_isas() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (homes, post, encoding) = staged_call_encoding(target);
        let selected_stage = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let selected = selected_stage.selected().selected_plan();
        let caller = selected
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let call_instructions = caller.blocks[0]
            .instructions
            .iter()
            .filter_map(|instruction| match instruction.kind {
                SelectedInstructionKind::CallI64 { callee } => Some((instruction.id, callee)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(call_instructions.len(), 3);
        assert_eq!(
            encoding.counts(),
            SelectedFormEncodingCounts {
                ordinary_encoded: 17,
                ordinary_deferred_control: 1,
                ordinary_encoded_call_templates: 3,
                ordinary_deferred_internal_control: 3,
                ordinary_internal_fixups: 3,
                structural_encoded_call_templates: 0,
                structural_encoded_returns: 0,
                structural_deferred_internal_control: 0,
                structural_internal_fixups: 0,
            }
        );

        for (instruction, callee) in &call_instructions {
            let row = encoding
                .rows()
                .iter()
                .find(|row| row.instruction == *instruction)
                .unwrap();
            let SelectedFormEncodingState::UnresolvedInternalMachineCall {
                bytes,
                footprint,
                fixup,
            } = &row.state
            else {
                panic!("ordinary scalar call must retain unresolved typed custody")
            };
            assert_eq!(fixup.callee, *callee);
            assert_eq!(
                fixup.state,
                SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
            );
            assert_eq!(fixup.addend, 0);
            assert_eq!(
                footprint.encoded,
                post.machine()
                    .plan()
                    .functions
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .flat_map(|block| &block.instructions)
                    .find(|machine| machine.instruction == *instruction)
                    .unwrap()
                    .alternative
                    .encoded
            );
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(bytes, &[0xe8, 0, 0, 0, 0]);
                    assert_eq!(
                        fixup.kind,
                        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
                    );
                    assert_eq!(
                        (
                            fixup.opcode_row_offset,
                            fixup.patch_row_offset,
                            fixup.reference_row_offset,
                            fixup.patch_byte_width
                        ),
                        (0, 1, 5, 4)
                    );
                }
                Architecture::Aarch64 => {
                    assert_eq!(bytes, &0x9400_0000_u32.to_le_bytes());
                    assert_eq!(
                        fixup.kind,
                        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
                    );
                    assert_eq!(
                        (
                            fixup.opcode_row_offset,
                            fixup.patch_row_offset,
                            fixup.reference_row_offset,
                            fixup.patch_byte_width
                        ),
                        (0, 0, 0, 4)
                    );
                }
            }
        }

        let physical = selected_stage.register_environment().physical();
        validate_optimized_layout_independent_selected_form_encoding(
            selected_stage.selected(),
            &post,
            physical,
            &encoding,
        )
        .unwrap();
        let layout = stage_optimized_resolved_selected_form_layout(
            selected_stage.selected(),
            &post,
            physical,
            &encoding,
        )
        .unwrap();
        validate_optimized_resolved_selected_form_layout(
            selected_stage.selected(),
            &post,
            physical,
            &encoding,
            &layout,
        )
        .unwrap();
        let caller_layout = layout
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let layout_calls = caller_layout.blocks[0]
            .instructions
            .iter()
            .filter(|row| {
                call_instructions
                    .iter()
                    .any(|(id, _)| *id == row.instruction)
            })
            .collect::<Vec<_>>();
        assert_eq!(layout_calls.len(), 3);
        assert!(
            layout_calls
                .windows(2)
                .all(|rows| rows[0].offset < rows[1].offset)
        );
        for row in layout_calls {
            let encoded = encoding
                .rows()
                .iter()
                .find(|encoded| encoded.instruction == row.instruction)
                .unwrap();
            let SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup, .. } =
                &encoded.state
            else {
                unreachable!()
            };
            assert_eq!(row.internal_machine_fixup, Some(*fixup));
            assert!(row.branch.is_none());
        }

        let error = stage_whole_function_exit_contract(
            selected_stage.selected(),
            &post,
            physical,
            &encoding,
            &layout,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            WholeFunctionExitContractError::CalleeSavedWrite { .. }
                | WholeFunctionExitContractError::NonReturnStackEffect(_)
                | WholeFunctionExitContractError::LinkRegisterWrite(_)
        ));
    }
}

#[test]
fn selected_call_template_and_layout_corruption_fail_independent_replay() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (homes, post, encoding) = staged_call_encoding(target);
        let selected_stage = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let physical = selected_stage.register_environment().physical();
        let selected = selected_stage.selected();
        let call_index = encoding
            .rows()
            .iter()
            .position(|row| {
                matches!(
                    row.state,
                    SelectedFormEncodingState::UnresolvedInternalMachineCall { .. }
                )
            })
            .unwrap();

        for mutation in 0..6 {
            let mut corrupted = encoding.clone();
            let SelectedFormEncodingState::UnresolvedInternalMachineCall {
                bytes,
                footprint,
                fixup,
            } = &mut corrupted.rows_mut()[call_index].state
            else {
                unreachable!()
            };
            match mutation {
                0 => bytes[0] ^= 1,
                1 => fixup.callee = MachineId::new(SCALAR_CALL_UNIT_CALLEE_BASE + 2).unwrap(),
                2 => fixup.patch_row_offset += 1,
                3 => fixup.reference_row_offset += 1,
                4 => fixup.addend = 1,
                5 => footprint.register_reads.clear(),
                _ => unreachable!(),
            }
            assert!(matches!(
                validate_optimized_layout_independent_selected_form_encoding(
                    selected, &post, physical, &corrupted,
                ),
                Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
            ));
        }

        let layout =
            stage_optimized_resolved_selected_form_layout(selected, &post, physical, &encoding)
                .unwrap();
        let mut corrupted = layout.clone();
        let call = corrupted
            .functions_mut()
            .iter_mut()
            .flat_map(|function| &mut function.blocks)
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.internal_machine_fixup.is_some())
            .unwrap();
        call.internal_machine_fixup
            .as_mut()
            .unwrap()
            .patch_row_offset += 1;
        assert!(matches!(
            validate_optimized_resolved_selected_form_layout(
                selected, &post, physical, &encoding, &corrupted,
            ),
            Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
        ));
    }
}
