use crate::tests::*;

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
        None,
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
                    selected, &post, physical, None, &corrupted,
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
