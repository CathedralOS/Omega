use crate::tests::*;

pub(super) fn verify_structural_call_encoding_and_layout(homes: &StagedOptimizedRegisterHomes) {
    let legality_stage = homes.legality_stage();
    let range_stage = legality_stage.live_range_stage();
    let environment = range_stage
        .liveness_stage()
        .selected_stage()
        .register_environment();

    let post = stage_optimized_post_allocation_machine_plan(homes)
        .expect("structural call must reach post-allocation machine custody");
    assert!(post.machine().plan().functions.is_empty());
    assert_eq!(post.machine().plan().structural_unit_functions.len(), 2);
    assert!(
        post.machine().plan().structural_unit_functions[0]
            .call
            .is_some()
    );
    assert!(
        post.machine().plan().structural_unit_functions[1]
            .call
            .is_none()
    );
    assert_eq!(post.custody().function_count(), 0);
    assert_eq!(post.custody().structural_unit_function_count(), 2);
    assert_eq!(post.machine().receipt().function_count(), 2);
    assert_eq!(post.machine().receipt().block_count(), 2);
    assert_eq!(post.machine().receipt().instruction_count(), 3);
    assert_eq!(post.machine().receipt().operand_count(), 0);
    let physical_program = post.machine().plan();
    let call = physical_program.structural_unit_functions[0]
        .call
        .as_ref()
        .unwrap();
    let return_actions = physical_program
        .structural_unit_functions
        .iter()
        .map(|function| {
            let instruction = &function.return_instruction;
            instruction.unit_uses.len()
                + instruction.unit_defs.len()
                + instruction.unit_clobbers.len()
        })
        .sum::<usize>();
    assert_eq!(
        post.machine().receipt().unit_action_count(),
        return_actions + call.unit_uses.len() + call.unit_defs.len() + call.unit_clobbers.len()
    );
    let mut corrupted = post.machine().plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .unit_uses
        .clear();
    // Canonical encoding authenticates data; it cannot admit a different call.
    corrupted.identity = physical_instructions::post_allocation_machine_identity(&corrupted);
    let corrupted = physical_instructions::PostAllocationMachinePlan::decode(&corrupted.encode())
        .expect("substituted data has a valid canonical frame, not realization authority");
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

    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        range_stage.liveness_stage().selected_stage().selected(),
        &post,
        environment.physical(),
    )
    .expect("structural Unit calls must retain typed unresolved pre-layout encoding");
    assert!(encoding.rows().is_empty());
    assert_eq!(encoding.structural_unit_functions().len(), 2);
    assert_eq!(
        encoding.counts(),
        SelectedFormEncodingCounts {
            ordinary_encoded: 0,
            ordinary_deferred_control: 0,
            ordinary_encoded_call_templates: 0,
            ordinary_deferred_internal_control: 0,
            ordinary_internal_fixups: 0,
            structural_encoded_call_templates: 1,
            structural_encoded_returns: 2,
            structural_deferred_internal_control: 1,
            structural_internal_fixups: 1,
        }
    );
    let encoded_caller = &encoding.structural_unit_functions()[0];
    let encoded_callee = &encoding.structural_unit_functions()[1];
    let encoded_call = encoded_caller
        .call
        .as_ref()
        .expect("caller owns one unresolved structural call template");
    assert_eq!(encoded_call.bytes.len(), 89);
    assert_eq!(
        encoded_call.callee,
        post.machine().plan().structural_unit_functions[0]
            .call
            .as_ref()
            .unwrap()
            .callee
    );
    assert_eq!(encoded_call.fixup.callee, encoded_call.callee);
    assert_eq!(encoded_call.fixup.opcode_byte_offset, 80);
    assert_eq!(encoded_call.fixup.field_byte_offset, 81);
    assert_eq!(encoded_call.fixup.next_instruction_byte_offset, 85);
    assert_eq!(encoded_call.fixup.field_byte_width, 4);
    assert_eq!(&encoded_call.bytes[81..85], &[0, 0, 0, 0]);
    assert!(encoded_callee.call.is_none());
    for function in [encoded_caller, encoded_callee] {
        assert!(matches!(
            &function.return_instruction.state,
            SelectedFormEncodingState::Encoded { bytes, .. }
                if bytes.as_slice() == [0xc3]
        ));
    }
    validate_optimized_layout_independent_selected_form_encoding(
        range_stage.liveness_stage().selected_stage().selected(),
        &post,
        environment.physical(),
        &encoding,
    )
    .unwrap();

    let mut corrupted = encoding.clone();
    corrupted.structural_unit_functions_mut()[0]
        .call
        .as_mut()
        .unwrap()
        .bytes[0] ^= 1;
    assert!(matches!(
        validate_optimized_layout_independent_selected_form_encoding(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &corrupted,
        ),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    ));
    let mut corrupted = encoding.clone();
    corrupted.counts_mut().structural_internal_fixups = 0;
    assert!(matches!(
        validate_optimized_layout_independent_selected_form_encoding(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &corrupted,
        ),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    ));

    let layout = stage_optimized_resolved_selected_form_layout(
        range_stage.liveness_stage().selected_stage().selected(),
        &post,
        environment.physical(),
        &encoding,
    )
    .expect("structural Unit fixups must reach unresolved function-relative custody");
    // Captured before separating layout data from its admitted stage wrapper.
    assert_eq!(
        layout.identity().bytes(),
        [
            138, 179, 115, 87, 187, 161, 49, 160, 196, 101, 233, 218, 171, 95, 59, 244, 117, 33,
            62, 122, 93, 132, 239, 112, 70, 30, 115, 7, 49, 14, 202, 212
        ]
    );
    assert_eq!(
        layout.policy(),
        SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1
    );
    assert!(layout.functions().is_empty());
    assert_eq!(layout.structural_unit_functions().len(), 2);
    let caller_layout = &layout.structural_unit_functions()[0];
    let callee_layout = &layout.structural_unit_functions()[1];
    assert_eq!((caller_layout.offset, caller_layout.byte_count), (0, 90));
    assert_eq!((callee_layout.offset, callee_layout.byte_count), (0, 1));
    let call_layout = caller_layout
        .call
        .as_ref()
        .expect("caller layout owns the unresolved template");
    assert_eq!(call_layout.offset, 0);
    assert_eq!(call_layout.bytes.len(), 89);
    assert_eq!(&call_layout.bytes[81..85], &[0, 0, 0, 0]);
    assert_eq!(call_layout.fixup, encoded_call.fixup);
    assert_eq!(caller_layout.return_instruction.offset, 89);
    assert_eq!(caller_layout.return_instruction.bytes, [0xc3]);
    assert_eq!(callee_layout.return_instruction.offset, 0);
    assert_eq!(callee_layout.return_instruction.bytes, [0xc3]);
    validate_optimized_resolved_selected_form_layout(
        range_stage.liveness_stage().selected_stage().selected(),
        &post,
        environment.physical(),
        &encoding,
        &layout,
    )
    .unwrap();
    let mut corrupted = layout.clone();
    corrupted.structural_unit_functions_mut()[0]
        .call
        .as_mut()
        .unwrap()
        .fixup
        .field_byte_offset += 1;
    assert!(matches!(
        validate_optimized_resolved_selected_form_layout(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &encoding,
            &corrupted,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    ));
    let mut corrupted = layout.clone();
    corrupted.structural_unit_functions_mut()[0]
        .call
        .as_mut()
        .unwrap()
        .bytes[0] ^= 1;
    assert!(matches!(
        validate_optimized_resolved_selected_form_layout(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &encoding,
            &corrupted,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    ));
    let mut corrupted = layout.clone();
    corrupted.structural_unit_functions_mut()[0]
        .return_instruction
        .offset += 1;
    assert!(matches!(
        validate_optimized_resolved_selected_form_layout(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &encoding,
            &corrupted,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    ));
    let mut corrupted = layout.clone();
    corrupted.structural_unit_functions_mut()[0].byte_count += 1;
    assert!(matches!(
        validate_optimized_resolved_selected_form_layout(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &encoding,
            &corrupted,
        ),
        Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
    ));

    let retained = layout.shared_program();
    assert!(std::ptr::eq(retained.as_ref(), layout.program()));
    assert!(std::sync::Arc::ptr_eq(
        &retained,
        &layout.clone().shared_program()
    ));
    drop(layout);
    assert_eq!(retained.identity, retained.recomputed_identity());
    let admitted = admit_resolved_machine_layout(
        range_stage.liveness_stage().selected_stage().selected(),
        &post,
        environment.physical(),
        &encoding,
        None,
        std::sync::Arc::clone(&retained),
    )
    .expect("retained current data must admit without the original layout wrapper");
    assert!(std::sync::Arc::ptr_eq(
        &retained,
        &admitted.shared_program()
    ));

    let original_byte = retained.structural_unit_functions[0]
        .call
        .as_ref()
        .unwrap()
        .bytes[0];
    let mut substituted = std::sync::Arc::clone(&retained);
    let changed = std::sync::Arc::make_mut(&mut substituted);
    changed.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .bytes[0] ^= 1;
    changed.identity = changed.recomputed_identity();
    assert_eq!(
        admitted.structural_unit_functions()[0]
            .call
            .as_ref()
            .unwrap()
            .bytes[0],
        original_byte
    );
    assert!(
        admit_resolved_machine_layout(
            range_stage.liveness_stage().selected_stage().selected(),
            &post,
            environment.physical(),
            &encoding,
            None,
            substituted,
        )
        .is_err(),
        "a recomputed identity must not admit substituted call bytes"
    );
}
