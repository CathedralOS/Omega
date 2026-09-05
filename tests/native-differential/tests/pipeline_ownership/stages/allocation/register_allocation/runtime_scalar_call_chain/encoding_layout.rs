use crate::FunctionFragmentReplayInputs;
use crate::tests::*;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use target::Architecture;

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

fn staged_frame(
    homes: &StagedOptimizedRegisterHomes,
    post: &StagedOptimizedPostAllocationMachinePlan,
) -> (
    ValidatedAllocatedCalleeSavedRequirements,
    ValidatedNonAuthoritativeCalleeSaveStorage,
    ValidatedTargetRegisterEnvironment,
    ValidatedTargetFrameLayout,
    ValidatedTargetFrameProtocolEncoding,
) {
    let environment = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .clone();
    let budget =
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap();
    let requirements = stage_allocated_callee_saved_requirements(
        homes,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        budget,
    )
    .unwrap();
    let storage = stage_non_authoritative_callee_save_storage(
        &requirements,
        &environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
    .unwrap();
    let frame = stage_target_frame_layout(
        post,
        &requirements,
        &storage,
        &environment,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )
    .unwrap();
    let protocol = stage_target_frame_protocol_encoding(
        &frame,
        &environment,
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
    )
    .unwrap();
    (requirements, storage, environment, frame, protocol)
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

        let (requirements, storage, environment, frame, protocol) = staged_frame(&homes, &post);
        assert_eq!(frame.receipt().function_count(), 2);
        assert_eq!(frame.receipt().calling_function_count(), 1);
        assert!(frame.receipt().callee_save_slot_count() >= 1);
        assert_eq!(frame.receipt().target(), target);
        assert_eq!(
            frame.receipt().identity(),
            target_frame_layout_identity(frame.plan())
        );
        assert_eq!(
            frame.receipt().post_allocation_machine(),
            post.machine().receipt().identity()
        );
        assert_eq!(
            frame.receipt().callee_save_storage(),
            storage.receipt().identity()
        );
        let caller_frame = frame
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let caller_storage = storage
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        assert!(caller_frame.contains_call);
        assert_eq!(caller_frame.pre_call_stack_alignment, 16);
        assert_eq!(
            caller_frame.callee_save_slots.len(),
            caller_storage.slots.len()
        );
        assert!(!caller_frame.callee_save_slots.is_empty());
        assert_eq!(caller_frame.callee_save_slots[0].frame_offset_bytes, 0);
        match target.architecture {
            Architecture::X86_64 => {
                assert!(caller_frame.frame_size_bytes >= caller_storage.abstract_area_bytes);
                assert_eq!(caller_frame.frame_size_bytes % 16, 8);
                assert_eq!(frame.receipt().saved_link_count(), 0);
                assert_eq!(
                    caller_frame.return_address,
                    ReturnAddressFrameCustody::CallerActivationStack {
                        post_prologue_offset_bytes: caller_frame.frame_size_bytes,
                        size_bytes: 8,
                    }
                );
            }
            Architecture::Aarch64 => {
                assert!(caller_frame.frame_size_bytes >= caller_storage.abstract_area_bytes + 8);
                assert_eq!(caller_frame.frame_size_bytes % 16, 0);
                assert_eq!(frame.receipt().saved_link_count(), 1);
                assert!(matches!(
                    caller_frame.return_address,
                    ReturnAddressFrameCustody::SavedLinkRegister {
                        frame_offset_bytes,
                        size_bytes: 8,
                        ..
                    } if frame_offset_bytes == caller_storage.abstract_area_bytes
                ));
            }
        }
        validate_target_frame_layout(
            &post,
            &requirements,
            &storage,
            &environment,
            frame.plan().clone(),
        )
        .unwrap();
        assert_eq!(
            protocol.receipt().identity(),
            target_frame_protocol_encoding_identity(protocol.plan())
        );
        assert_eq!(
            protocol.receipt().frame_layout(),
            frame.receipt().identity()
        );
        assert_eq!(protocol.receipt().function_count(), 2);
        assert!(protocol.receipt().byte_count() > 0);
        let caller_protocol = protocol
            .plan()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        let prologue = caller_protocol
            .prologue
            .bytes(&protocol.plan().bytes)
            .unwrap();
        let epilogue = caller_protocol
            .epilogue
            .bytes(&protocol.plan().bytes)
            .unwrap();
        assert!(!prologue.is_empty());
        assert!(!epilogue.is_empty());
        match target.architecture {
            Architecture::X86_64 => {
                assert_eq!(&prologue[..3], &[0x48, 0x83, 0xec]);
                assert_eq!(
                    &epilogue[epilogue.len() - 3..epilogue.len() - 1],
                    &[0x83, 0xc4]
                );
            }
            Architecture::Aarch64 => {
                let prologue_word = u32::from_le_bytes(prologue[..4].try_into().unwrap());
                let epilogue_word =
                    u32::from_le_bytes(epilogue[epilogue.len() - 4..].try_into().unwrap());
                assert_eq!(prologue_word & 0xffc0_03ff, 0xd100_03ff);
                assert_eq!(epilogue_word & 0xffc0_03ff, 0x9100_03ff);
            }
        }
        validate_target_frame_protocol_encoding(&frame, &environment, protocol.plan().clone())
            .unwrap();

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

        let contract = stage_whole_function_exit_contract_with_frame(
            selected_stage.selected(),
            &post,
            physical,
            &encoding,
            &layout,
            &frame,
            &protocol,
        )
        .unwrap();
        validate_whole_function_exit_contract_with_frame(
            selected_stage.selected(),
            &post,
            physical,
            &encoding,
            &layout,
            &frame,
            &protocol,
            &contract,
        )
        .unwrap();
        assert_eq!(
            contract.contract().frame,
            WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: frame.receipt().identity(),
                protocol: protocol.receipt().identity(),
            }
        );
        assert_eq!(
            contract.contract().policy,
            match target.architecture {
                Architecture::X86_64 => WholeFunctionExitPolicy::SystemVAMD64CanonicalFixedFrameV1,
                Architecture::Aarch64 => WholeFunctionExitPolicy::Aapcs64CanonicalFixedFrameV1,
            }
        );
        let caller_exit = contract
            .contract()
            .functions
            .iter()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        assert!(!caller_exit.modified_callee_saved_units.is_empty());

        let frame_budget =
            OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000)
                .unwrap();
        let realization = crate::tests::with_allocated_machine(
            homes.try_into().unwrap(),
            |allocation, machine| {
                stage_fixed_frame_function_relative_realization(allocation, machine, frame_budget)
            },
        )
        .unwrap();
        validate_fixed_frame_function_relative_realization(&realization).unwrap();
        assert_eq!(
            realization.manifest().record().frame,
            FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
                layout: realization.frame().receipt().identity(),
                protocol: realization.protocol().receipt().identity(),
            }
        );
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &realization.manifest().record().encode()
            ),
            Ok(realization.manifest().record().clone())
        );
        let fragments = stage_optimized_function_fragment_emission(
            FunctionFragmentReplayInputs::FixedFrame(Box::new(realization)).into(),
        )
        .unwrap();
        assert_eq!(
            fragments.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1
        );
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&fragments.manifest().record().encode()),
            Ok(fragments.manifest().record().clone())
        );
        assert_eq!(
            fragments
                .fragments()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .filter(|row| row.internal_machine_fixup.is_some())
                .count(),
            3
        );
        let source_bytes = fragments
            .fragments()
            .functions
            .iter()
            .map(|row| row.byte_count)
            .sum::<u64>();
        let applied = stage_function_fragment_frame_application(fragments).unwrap();
        validate_function_fragment_frame_application(&applied).unwrap();
        assert!(
            applied
                .fragments()
                .functions
                .iter()
                .map(|row| row.byte_count)
                .sum::<u64>()
                > source_bytes
        );
        assert_eq!(applied.receipt().framed_function_count(), 1);
        let frame_application = applied.receipt().identity();
        let mut text = stage_optimized_fixed_frame_text_section(applied).unwrap();
        crate::tests::text_placement_checks::fixed(&text);
        validate_optimized_fixed_frame_text_section(&text).unwrap();
        assert_eq!(
            text.manifest().record().source_custody,
            FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 {
                application: frame_application,
            }
        );
        assert_eq!(text.text_section().resolved_internal_machine_calls.len(), 3);
        assert_eq!(
            text.manifest()
                .record()
                .statistics
                .source_internal_machine_fixups,
            3
        );
        assert_eq!(
            text.manifest()
                .record()
                .statistics
                .resolved_internal_machine_fixups,
            3
        );
        assert_eq!(
            text.manifest()
                .record()
                .statistics
                .remaining_internal_machine_fixups,
            0
        );
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&text.manifest().record().encode()),
            Ok(text.manifest().record().clone())
        );
        for resolution in &text.text_section().resolved_internal_machine_calls {
            assert_eq!(
                i128::from(resolution.next_instruction_section_offset)
                    + i128::from(resolution.displacement),
                i128::from(resolution.callee_section_offset)
            );
        }
        text.text_section_mut().resolved_internal_machine_calls[0].displacement += 1;
        assert_eq!(
            validate_optimized_fixed_frame_text_section(&text),
            Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
        );
        text.text_section_mut().resolved_internal_machine_calls[0].displacement -= 1;
        let source_custody = text.manifest().record().source_custody;
        text.manifest_mut().record_mut().source_custody =
            FunctionFragmentTextSectionSourceCustody::DirectFragmentEmissionV1;
        assert_eq!(
            validate_optimized_fixed_frame_text_section(&text),
            Err(RelocationFreeTextSectionPlacementError::ManifestMismatch)
        );
        text.manifest_mut().record_mut().source_custody = source_custody;
        text.corrupt_custody_frame_application_for_test();
        assert_eq!(
            validate_optimized_fixed_frame_text_section(&text),
            Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch)
        );
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

        let (requirements, storage, environment, frame, protocol) = staged_frame(&homes, &post);
        for corrupt in [
            |plan: &mut TargetFrameLayoutPlan| plan.functions[0].frame_size_bytes += 8,
            |plan: &mut TargetFrameLayoutPlan| {
                plan.functions[0].callee_save_slots[0].frame_offset_bytes += 8
            },
            |plan: &mut TargetFrameLayoutPlan| plan.functions[0].contains_call = false,
            |plan: &mut TargetFrameLayoutPlan| plan.functions.clear(),
            |plan: &mut TargetFrameLayoutPlan| plan.functions.push(plan.functions[0].clone()),
            |plan: &mut TargetFrameLayoutPlan| plan.functions.swap(0, 1),
            |plan: &mut TargetFrameLayoutPlan| plan.functions[0].pre_call_stack_alignment = 8,
            |plan: &mut TargetFrameLayoutPlan| plan.functions[0].abi_stack_alignment_bytes = 8,
            |plan: &mut TargetFrameLayoutPlan| plan.functions[0].callee_save_slots.clear(),
            |plan: &mut TargetFrameLayoutPlan| {
                plan.functions[0].callee_save_slots[0].size_bytes += 1
            },
            |plan: &mut TargetFrameLayoutPlan| {
                plan.functions[0].callee_save_slots[0].alignment_bytes += 1
            },
            |plan: &mut TargetFrameLayoutPlan| plan.functions[0].frame_size_bytes += 16,
        ] {
            let mut changed = frame.plan().clone();
            corrupt(&mut changed);
            assert_eq!(
                validate_target_frame_layout(&post, &requirements, &storage, &environment, changed,),
                Err(TargetFrameLayoutError::NonCanonicalLayout)
            );
        }

        for corrupt in [
            |plan: &mut TargetFrameProtocolEncodingPlan| plan.bytes[0] ^= 1,
            |plan: &mut TargetFrameProtocolEncodingPlan| plan.bytes.push(0),
            |plan: &mut TargetFrameProtocolEncodingPlan| {
                plan.bytes.pop();
            },
            |plan: &mut TargetFrameProtocolEncodingPlan| plan.functions.clear(),
            |plan: &mut TargetFrameProtocolEncodingPlan| plan.functions.push(plan.functions[0]),
            |plan: &mut TargetFrameProtocolEncodingPlan| plan.functions.swap(0, 1),
            |plan: &mut TargetFrameProtocolEncodingPlan| plan.functions[0].prologue.offset += 1,
            |plan: &mut TargetFrameProtocolEncodingPlan| {
                plan.functions[0].epilogue = plan.functions[0].prologue
            },
        ] {
            let mut changed = protocol.plan().clone();
            corrupt(&mut changed);
            assert_eq!(
                validate_target_frame_protocol_encoding(&frame, &environment, changed),
                Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding)
            );
        }

        let mut changed = protocol.plan().clone();
        let caller = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine())
            .unwrap();
        caller.prologue.length -= 1;
        assert_eq!(
            validate_target_frame_protocol_encoding(&frame, &environment, changed),
            Err(TargetFrameProtocolEncodingError::NonCanonicalEncoding)
        );

        let mut changed = protocol.plan().clone();
        changed.frame_layout = TargetFrameLayoutIdentity::from_bytes([0xa7; 32]);
        assert_eq!(
            validate_target_frame_protocol_encoding(&frame, &environment, changed),
            Err(TargetFrameProtocolEncodingError::RootMismatch)
        );
    }
}
