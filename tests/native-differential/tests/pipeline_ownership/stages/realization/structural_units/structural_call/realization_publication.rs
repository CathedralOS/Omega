use crate::FunctionFragmentReplayInputs;
use crate::tests::*;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

pub(super) fn realize_and_publish_structural_call(homes: StagedOptimizedRegisterHomes) {
    let current = homes.replay_allocation().unwrap();
    let selected_owner = current.selected().shared_selected_plan();
    let home_owner = current.homes().shared_plan();
    let mut realization =
        crate::tests::with_allocated_machine(homes.try_into().unwrap(), |allocation, machine| {
            stage_fixed_frame_function_relative_realization(
                allocation,
                machine,
                selected_lowering_budget(),
            )
        })
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &selected_owner,
        &realization.allocation().program().selected
    ));
    assert!(std::sync::Arc::ptr_eq(
        &home_owner,
        &realization.allocation().program().homes
    ));
    let original = realization.allocation().program().clone();
    for replace_selected in [false, true] {
        let mut changed = original.clone();
        if replace_selected {
            std::sync::Arc::make_mut(&mut changed.selected)
                .functions
                .clear();
        } else {
            std::sync::Arc::make_mut(&mut changed.homes)
                .functions
                .clear();
        }
        realization
            .allocation_mut()
            .substitute_current_program_for_test(changed);
        assert!(matches!(
            validate_fixed_frame_function_relative_realization(&realization),
            Err(FunctionRelativeOptimizationRealizationError::Allocation(
                AllocationReplayError::CurrentProgramMismatch
            ))
        ));
        realization
            .allocation_mut()
            .substitute_current_program_for_test(original.clone());
    }
    let exit = realization.exit_contract().contract();
    assert_eq!(exit.functions.len(), 2);
    assert!(exit.functions.iter().all(|function| {
        function.body_stack_delta == 0
            && function
                .returns
                .iter()
                .all(|returned| returned.value == WholeFunctionReturnValueEvidence::UnitV1)
    }));
    let original_exit = realization.exit_contract().shared_contract();
    for mutation in 0..6 {
        let changed = realization.exit_contract_mut().contract_mut();
        match mutation {
            0 => changed.functions.reverse(),
            1 => {
                changed.functions.pop();
            }
            2 => changed.functions[0].body_stack_delta += 8,
            3 => changed.functions[0].returns[0].offset += 1,
            4 => changed.functions[1].returns[0].bytes[0] ^= 1,
            _ => changed.functions[0].returns.clear(),
        }
        changed.identity = changed.recomputed_identity();
        assert!(
            validate_fixed_frame_function_relative_realization(&realization).is_err(),
            "exit mutation {mutation}"
        );
        *realization.exit_contract_mut().contract_mut() = (*original_exit).clone();
    }
    let original_manifest = realization.manifest().record().clone();
    realization
        .manifest_mut()
        .record_mut()
        .statistics
        .unresolved_internal_machine_fixups = 0;
    assert!(validate_fixed_frame_function_relative_realization(&realization).is_err());
    *realization.manifest_mut().record_mut() = original_manifest;
    validate_fixed_frame_function_relative_realization(&realization).unwrap();

    let mut fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::FixedFrame(Box::new(realization)).into(),
    )
    .unwrap();
    assert_eq!(fragments.fragments().functions.len(), 2);
    let original_fragments = fragments.fragments().clone();
    for mutation in 0..8 {
        let mut changed = original_fragments.clone();
        match mutation {
            0 => changed.functions.reverse(),
            1..=3 => {
                let row = changed.functions[0]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| row.internal_machine_fixup.is_some())
                    .unwrap();
                if mutation == 1 {
                    row.internal_machine_fixup = None;
                } else if mutation == 2 {
                    row.internal_machine_fixup
                        .as_mut()
                        .unwrap()
                        .patch_function_offset += 1;
                } else {
                    row.bytes[0] ^= 1;
                }
            }
            4 => {
                changed.functions[0].blocks[0]
                    .instructions
                    .last_mut()
                    .unwrap()
                    .offset += 1
            }
            5 => {
                changed.functions[1].blocks[0]
                    .instructions
                    .last_mut()
                    .unwrap()
                    .control = machine_code::FunctionFragmentControlProvenance::None
            }
            6 => changed.functions[1].bytes[0] ^= 1,
            _ => changed.functions[1].machine = changed.functions[0].machine,
        }
        changed.identity = changed.recomputed_identity();
        assert_ne!(changed.identity, original_fragments.identity);
        assert!(
            machine_emission::validate_resolved_function_fragments(
                fragments.source().program(),
                &changed
            )
            .is_err(),
            "fragment mutation {mutation}"
        );
        *fragments.fragments_mut() = changed;
        assert!(validate_optimized_function_fragment_emission(&fragments).is_err());
        *fragments.fragments_mut() = original_fragments.clone();
    }
    let fragment_manifest = fragments.manifest().record();
    assert_eq!(
        fragment_manifest.source_kind,
        FunctionFragmentEmissionSourceKind::CanonicalFixedFrameBodyV1
    );
    assert_eq!(fragment_manifest.statistics.functions, 2);
    assert_eq!(
        fragment_manifest
            .statistics
            .unresolved_internal_machine_fixups,
        1
    );
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&fragment_manifest.encode()),
        Ok(fragment_manifest.clone())
    );
    for unsupported in [5_u32, 7, 12, 14] {
        let mut encoded = fragment_manifest.encode();
        encoded[8..12].copy_from_slice(&unsupported.to_le_bytes());
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&encoded),
            Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(unsupported))
        );
    }
    let applied = stage_function_fragment_frame_application(fragments).unwrap();
    validate_function_fragment_frame_application(&applied).unwrap();
    let mut text = stage_optimized_fixed_frame_text_section(applied).unwrap();
    crate::tests::text_placement_checks::fixed(&text);
    let placed = text.text_section();
    assert_eq!(placed.functions.len(), 2);
    assert_eq!(placed.resolved_internal_machine_calls.len(), 1);
    let call = placed.resolved_internal_machine_calls[0];
    assert_eq!(
        call.callee_section_offset,
        placed.functions[1].section_offset
    );
    assert_eq!(
        call.next_instruction_section_offset as i64 + i64::from(call.displacement),
        call.callee_section_offset as i64
    );
    assert_eq!(
        &placed.bytes[call.field_section_offset as usize..call.field_section_offset as usize + 4],
        &call.displacement.to_le_bytes()
    );
    assert_eq!(text.manifest().record().statistics.functions, 2);
    assert_eq!(
        text.manifest()
            .record()
            .statistics
            .source_internal_machine_fixups,
        1
    );
    assert_eq!(
        text.manifest()
            .record()
            .statistics
            .resolved_internal_machine_fixups,
        1
    );
    assert_eq!(
        text.manifest()
            .record()
            .statistics
            .remaining_internal_machine_fixups,
        0
    );
    let original_section = text.text_section().clone();
    text.text_section_mut().resolved_internal_machine_calls[0].displacement += 1;
    let identity = text.text_section().recomputed_identity();
    text.text_section_mut().identity = identity;
    assert!(validate_optimized_fixed_frame_text_section(&text).is_err());
    *text.text_section_mut() = original_section;
    let original_manifest = text.manifest().record().clone();
    text.manifest_mut()
        .record_mut()
        .statistics
        .resolved_internal_machine_fixups = 0;
    assert!(validate_optimized_fixed_frame_text_section(&text).is_err());
    *text.manifest_mut().record_mut() = original_manifest;
    validate_optimized_fixed_frame_text_section(&text).unwrap();
    let text_manifest = text.manifest().record();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&text_manifest.encode()),
        Ok(text_manifest.clone())
    );
    for unsupported in [5_u32, 7, 13, 15] {
        let mut encoded = text_manifest.encode();
        encoded[8..12].copy_from_slice(&unsupported.to_le_bytes());
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&encoded),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(unsupported))
        );
    }
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    assert_eq!(object.object().symbols.len(), 2);
    assert_eq!(object.object().relocation_record_count, 0);
    validate_optimized_relocation_free_object_container(&object).unwrap();
}
