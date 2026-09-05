use crate::FunctionFragmentReplayInputs;
use crate::tests::*;
use omega_regalloc::ValidatedSelectedAnalysis;

pub(super) fn realize_and_publish_structural_call(homes: StagedOptimizedRegisterHomes) {
    let current = homes.replay_allocation().unwrap();
    let selected_owner = current.selected().shared_selected_plan();
    let home_owner = current.homes().shared_plan();
    let mut realization =
        stage_optimized_structural_unit_function_relative_realization(homes.try_into().unwrap())
            .expect("structural Unit calls must reach owning function-relative custody");
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
                .structural_unit_functions
                .clear();
        } else {
            std::sync::Arc::make_mut(&mut changed.homes)
                .structural_unit_functions
                .clear();
        }
        realization
            .allocation_mut()
            .substitute_current_program_for_test(changed);
        assert!(matches!(
            validate_optimized_structural_unit_function_relative_realization(&realization),
            Err(
                OptimizedStructuralUnitFunctionRelativeRealizationError::Allocation(
                    AllocationReplayError::CurrentProgramMismatch
                )
            )
        ));
        realization
            .allocation_mut()
            .substitute_current_program_for_test(original.clone());
        validate_optimized_structural_unit_function_relative_realization(&realization).unwrap();
    }
    let exit = realization.exit_contract().contract();
    assert_eq!(
        exit.policy,
        WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1
    );
    assert!(exit.functions.is_empty());
    assert_eq!(exit.structural_unit_functions.len(), 2);
    assert_eq!(exit.structural_unit_functions[0].body_stack_delta, 0);
    assert!(
        exit.structural_unit_functions
            .iter()
            .all(|function| function.returned.value == WholeFunctionReturnValueEvidence::UnitV1)
    );
    let exit_call = exit.structural_unit_functions[0]
        .call
        .as_ref()
        .expect("entry caller retains whole-function call evidence");
    assert_eq!(exit_call.offset, 0);
    assert_eq!(exit_call.bytes.len(), 89);
    assert!(exit_call.frame_is_balanced);
    assert_eq!(exit_call.frame_byte_count, 72);
    assert_eq!(exit_call.shadow_byte_count, 32);
    assert_eq!(exit_call.pre_call_stack_alignment, 16);

    let original_exit = realization.exit_contract().shared_contract();
    for mutation in 0..7 {
        let changed = realization.exit_contract_mut().contract_mut();
        match mutation {
            0 => changed.structural_unit_functions.reverse(),
            1 => {
                changed.structural_unit_functions.pop();
            }
            2 => changed.structural_unit_functions[0].call = None,
            3 => {
                changed.structural_unit_functions[0]
                    .call
                    .as_mut()
                    .unwrap()
                    .fixup
                    .field_byte_offset += 1
            }
            4 => {
                changed.structural_unit_functions[0]
                    .call
                    .as_mut()
                    .unwrap()
                    .frame_is_balanced = false
            }
            5 => changed.structural_unit_functions[0].returned.offset += 1,
            6 => changed.structural_unit_functions[1].returned.bytes[0] ^= 1,
            _ => unreachable!(),
        }
        changed.identity = changed.recomputed_identity();
        assert!(
            matches!(
                validate_optimized_structural_unit_function_relative_realization(&realization),
                Err(
                    OptimizedStructuralUnitFunctionRelativeRealizationError::Exit(
                        WholeFunctionExitContractError::ArtifactMismatch
                    )
                )
            ),
            "structural exit mutation {mutation}"
        );
        *realization.exit_contract_mut().contract_mut() = (*original_exit).clone();
        validate_optimized_structural_unit_function_relative_realization(&realization).unwrap();
    }

    let manifest = realization.manifest().record();
    assert_eq!(manifest.statistics.functions, 0);
    assert_eq!(manifest.statistics.blocks, 0);
    assert_eq!(manifest.statistics.instructions, 0);
    assert_eq!(manifest.statistics.bytes, 0);
    assert_eq!(manifest.statistics.resolved_conditional_branches, 0);
    assert_eq!(manifest.statistics.structural_unit_functions, 2);
    assert_eq!(manifest.statistics.structural_unit_blocks, 2);
    assert_eq!(manifest.statistics.structural_unit_instructions, 3);
    assert_eq!(manifest.statistics.structural_unit_bytes, 91);
    assert_eq!(manifest.statistics.unresolved_internal_machine_fixups, 1);
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()),
        Ok(manifest.clone())
    );
    validate_optimized_structural_unit_function_relative_realization(&realization).unwrap();

    let original_offset = realization.layout().structural_unit_functions()[0]
        .return_instruction
        .offset;
    realization.layout_mut().structural_unit_functions_mut()[0]
        .return_instruction
        .offset = original_offset + 1;
    assert!(matches!(
        validate_optimized_structural_unit_function_relative_realization(&realization),
        Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::Layout(
                OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch
            )
        )
    ));
    realization.layout_mut().structural_unit_functions_mut()[0]
        .return_instruction
        .offset = original_offset;
    validate_optimized_structural_unit_function_relative_realization(&realization).unwrap();

    realization
        .exit_contract_mut()
        .contract_mut()
        .structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .frame_is_balanced = false;
    assert!(matches!(
        validate_optimized_structural_unit_function_relative_realization(&realization),
        Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::Exit(
                WholeFunctionExitContractError::ArtifactMismatch
            )
        )
    ));
    realization
        .exit_contract_mut()
        .contract_mut()
        .structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .frame_is_balanced = true;
    validate_optimized_structural_unit_function_relative_realization(&realization).unwrap();

    realization
        .manifest_mut()
        .record_mut()
        .statistics
        .unresolved_internal_machine_fixups = 0;
    assert!(matches!(
        validate_optimized_structural_unit_function_relative_realization(&realization),
        Err(OptimizedStructuralUnitFunctionRelativeRealizationError::RootMismatch)
    ));
    realization
        .manifest_mut()
        .record_mut()
        .statistics
        .unresolved_internal_machine_fixups = 1;
    validate_optimized_structural_unit_function_relative_realization(&realization).unwrap();

    let mut fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::StructuralUnit(Box::new(realization)).into(),
    )
    .expect("structural Unit calls must retain typed unresolved fragment custody");
    assert!(fragments.fragments().functions.is_empty());
    assert_eq!(fragments.fragments().structural_unit_functions.len(), 2);
    let original_fragments = fragments.fragments().clone();
    for mutation in 0..7 {
        let mut changed = original_fragments.clone();
        match mutation {
            0 => changed.structural_unit_functions.reverse(),
            1 => changed.structural_unit_functions[0].block.call = None,
            2 => {
                changed.structural_unit_functions[0]
                    .block
                    .call
                    .as_mut()
                    .unwrap()
                    .fixup
                    .patch_function_offset += 1
            }
            3 => {
                changed.structural_unit_functions[0]
                    .block
                    .call
                    .as_mut()
                    .unwrap()
                    .bytes[0] ^= 1
            }
            4 => {
                changed.structural_unit_functions[0]
                    .block
                    .return_instruction
                    .offset += 1
            }
            5 => {
                changed.structural_unit_functions[1]
                    .block
                    .return_instruction
                    .control = omega_machine_code::FunctionFragmentControlProvenance::None
            }
            6 => changed.structural_unit_functions[1].bytes[0] ^= 1,
            _ => unreachable!(),
        }
        changed.identity = changed.recomputed_identity();
        assert_ne!(changed.identity, original_fragments.identity);
        assert_eq!(
            omega_machine_emission::validate_resolved_function_fragments(
                fragments.source().program(),
                &changed
            ),
            Err(omega_machine_emission::ResolvedFragmentEmissionError::ArtifactMismatch),
            "structural projection mutation {mutation}"
        );
    }
    let caller_fragment = &fragments.fragments().structural_unit_functions[0];
    let callee_fragment = &fragments.fragments().structural_unit_functions[1];
    assert_eq!(
        (caller_fragment.byte_count, callee_fragment.byte_count),
        (90, 1)
    );
    assert_eq!(caller_fragment.bytes.len(), 90);
    assert_eq!(&caller_fragment.bytes[81..85], &[0, 0, 0, 0]);
    assert_eq!(caller_fragment.bytes[89], 0xc3);
    assert_eq!(callee_fragment.bytes, [0xc3]);
    let fragment_call = caller_fragment
        .block
        .call
        .as_ref()
        .expect("caller fragment owns the unresolved internal call");
    assert_eq!(fragment_call.offset, 0);
    assert_eq!(fragment_call.fixup.opcode_function_offset, 80);
    assert_eq!(fragment_call.fixup.patch_function_offset, 81);
    assert_eq!(fragment_call.fixup.reference_function_offset, 85);
    assert_eq!(fragment_call.fixup.patch_byte_width, 4);
    assert_eq!(fragment_call.fixup.addend, 0);
    let fragment_manifest = fragments.manifest().record();
    assert_eq!(
        fragment_manifest.stage,
        FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1
    );
    assert_eq!(
        fragment_manifest.source_kind,
        FunctionFragmentEmissionSourceKind::StructuralUnitV1
    );
    assert_eq!(fragment_manifest.statistics.functions, 0);
    assert_eq!(fragment_manifest.statistics.structural_unit_functions, 2);
    assert_eq!(fragment_manifest.statistics.structural_unit_blocks, 2);
    assert_eq!(
        fragment_manifest
            .statistics
            .structural_unit_instruction_spans,
        3
    );
    assert_eq!(fragment_manifest.statistics.structural_unit_bytes, 91);
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
    for unsupported in [5_u32, 7_u32, 12_u32] {
        let mut encoded = fragment_manifest.encode();
        encoded[8..12].copy_from_slice(&unsupported.to_le_bytes());
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&encoded),
            Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(unsupported))
        );
    }
    validate_optimized_function_fragment_emission(&fragments).unwrap();
    let original_field_offset = fragments.fragments().structural_unit_functions[0]
        .block
        .call
        .as_ref()
        .unwrap()
        .fixup
        .patch_function_offset;
    fragments.fragments_mut().structural_unit_functions[0]
        .block
        .call
        .as_mut()
        .unwrap()
        .fixup
        .patch_function_offset += 1;
    assert!(matches!(
        validate_optimized_function_fragment_emission(&fragments),
        Err(FunctionFragmentEmissionError::ArtifactMismatch)
    ));
    assert!(matches!(
        crate::stages::artifacts::function_fragment_text_section::place_structural_unit_fragments_for_test(&fragments),
        Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)
    ));
    fragments.fragments_mut().structural_unit_functions[0]
        .block
        .call
        .as_mut()
        .unwrap()
        .fixup
        .patch_function_offset = original_field_offset;
    validate_optimized_function_fragment_emission(&fragments).unwrap();

    let original_template_byte = fragments.fragments().structural_unit_functions[0]
        .block
        .call
        .as_ref()
        .unwrap()
        .bytes[0];
    fragments.fragments_mut().structural_unit_functions[0]
        .block
        .call
        .as_mut()
        .unwrap()
        .bytes[0] ^= 1;
    assert!(matches!(
        crate::stages::artifacts::function_fragment_text_section::place_structural_unit_fragments_for_test(&fragments),
        Err(
            RelocationFreeTextSectionPlacementError::StructuralUnitCallTemplate(
                _,
                omega_isa_x86_64::X86_64StructuralUnitCallTemplateError::MalformedTemplate
            )
        )
    ));
    fragments.fragments_mut().structural_unit_functions[0]
        .block
        .call
        .as_mut()
        .unwrap()
        .bytes[0] = original_template_byte;
    validate_optimized_function_fragment_emission(&fragments).unwrap();

    let callee_machine = fragments.fragments().structural_unit_functions[1].machine;
    let caller_machine = fragments.fragments().structural_unit_functions[0].machine;
    fragments.fragments_mut().structural_unit_functions[1].machine = caller_machine;
    assert!(matches!(
        crate::stages::artifacts::function_fragment_text_section::place_structural_unit_fragments_for_test(
            &fragments
        ),
        Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(machine))
            if machine == caller_machine
    ));
    fragments.fragments_mut().structural_unit_functions[1].machine = callee_machine;
    validate_optimized_function_fragment_emission(&fragments).unwrap();

    let mut text = stage_optimized_relocation_free_text_section(fragments)
        .expect("whole-text placement must discharge the internal MachineId call");
    let placed = text.text_section();
    assert_eq!(placed.byte_count, 91);
    assert_eq!(placed.functions.len(), 2);
    assert_eq!(
        (
            placed.functions[0].section_offset,
            placed.functions[0].byte_count,
            placed.functions[1].section_offset,
            placed.functions[1].byte_count,
        ),
        (0, 90, 90, 1)
    );
    assert_eq!(&placed.bytes[81..85], &[5, 0, 0, 0]);
    assert_eq!(placed.bytes[89], 0xc3);
    assert_eq!(placed.bytes[90], 0xc3);
    assert_eq!(placed.resolved_internal_machine_calls.len(), 1);
    let resolved_call = placed.resolved_internal_machine_calls[0];
    assert_eq!(resolved_call.call_section_offset, 0);
    assert_eq!(resolved_call.opcode_section_offset, 80);
    assert_eq!(resolved_call.field_section_offset, 81);
    assert_eq!(resolved_call.next_instruction_section_offset, 85);
    assert_eq!(resolved_call.callee_section_offset, 90);
    assert_eq!(resolved_call.displacement, 5);
    assert_eq!(
        placed.relocation_requirements,
        omega_object_file::TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
    );
    let text_manifest = text.manifest().record();
    assert_eq!(text_manifest.statistics.functions, 0);
    assert_eq!(text_manifest.statistics.bytes, 0);
    assert_eq!(text_manifest.statistics.structural_unit_functions, 2);
    assert_eq!(text_manifest.statistics.structural_unit_blocks, 2);
    assert_eq!(
        text_manifest.statistics.structural_unit_instruction_spans,
        3
    );
    assert_eq!(text_manifest.statistics.structural_unit_bytes, 91);
    assert_eq!(text_manifest.statistics.source_internal_machine_fixups, 1);
    assert_eq!(text_manifest.statistics.resolved_internal_machine_fixups, 1);
    assert_eq!(
        text_manifest.statistics.remaining_internal_machine_fixups,
        0
    );
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&text_manifest.encode()),
        Ok(text_manifest.clone())
    );
    for unsupported in [5_u32, 7_u32, 12_u32] {
        let mut encoded = text_manifest.encode();
        encoded[8..12].copy_from_slice(&unsupported.to_le_bytes());
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&encoded),
            Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(unsupported))
        );
    }
    validate_optimized_relocation_free_text_section(&text).unwrap();
    text.text_section_mut().resolved_internal_machine_calls[0].displacement += 1;
    assert!(matches!(
        validate_optimized_relocation_free_text_section(&text),
        Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
    ));
    text.text_section_mut().resolved_internal_machine_calls[0].displacement = 5;
    validate_optimized_relocation_free_text_section(&text).unwrap();
    text.manifest_mut()
        .record_mut()
        .statistics
        .resolved_internal_machine_fixups = 0;
    assert!(matches!(
        validate_optimized_relocation_free_text_section(&text),
        Err(RelocationFreeTextSectionPlacementError::ManifestMismatch)
    ));
    text.manifest_mut()
        .record_mut()
        .statistics
        .resolved_internal_machine_fixups = 1;
    validate_optimized_relocation_free_text_section(&text).unwrap();

    let object = stage_optimized_relocation_free_object_container(text)
        .expect("resolved structural text must require no object relocation");
    assert_eq!(object.object().text_section.byte_count, 91);
    assert_eq!(&object.object().text_section.bytes[81..85], &[5, 0, 0, 0]);
    assert_eq!(object.object().symbols.len(), 2);
    assert_eq!(object.object().symbols[0].section_offset, 0);
    assert_eq!(object.object().symbols[0].byte_count, 90);
    assert_eq!(object.object().symbols[1].section_offset, 90);
    assert_eq!(object.object().symbols[1].byte_count, 1);
    assert_eq!(object.object().relocation_record_count, 0);
    validate_optimized_relocation_free_object_container(&object).unwrap();
}
