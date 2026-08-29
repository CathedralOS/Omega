use crate::tests::*;

#[test]
fn structural_unit_call_reaches_post_allocation_machine_custody() {
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
    let selected_call_uses = selected_call.implicit_uses.clone();
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

    let effects = stage_optimized_machine_effects(&selected)
        .expect("structural call must reach pre-allocation effect custody");
    assert_eq!(effects.effects().plan().structural_unit_functions.len(), 2);
    let effect_call = effects.effects().plan().structural_unit_functions[0]
        .call
        .as_ref()
        .expect("caller effect roster owns the atomic call");
    assert_eq!(effect_call.callee, selected_call.callee);
    assert_eq!(effect_call.unit_uses, selected_call.implicit_uses);
    assert_eq!(effect_call.effect, selected_call.effect);
    assert_eq!(effect_call.ownership, selected_call.ownership);
    assert_eq!(
        effect_call.declaration.frame,
        omega_selected_instructions::StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: 72,
            shadow_byte_count: 32,
            pre_call_stack_alignment: 16,
        }
    );
    assert_eq!(
        &validate_optimized_machine_effect_custody(&selected, effects.effects()).unwrap(),
        effects.custody()
    );

    let mut corrupted = effects.effects().plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .unit_uses
        .clear();
    let environment = selected.register_environment();
    let catalog = omega_isa_x86_64::validate_x86_64_machine_effect_catalog(
        NativeTarget::uefi_x64(),
        environment.constraints(),
        omega_isa_x86_64::x86_64_machine_effect_catalog(
            NativeTarget::uefi_x64(),
            environment.constraints(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        omega_machine_optimizer::validate_pre_allocation_machine_effects(
            selected.selected(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            &catalog,
            corrupted,
        )
        .is_err()
    );

    let liveness = stage_optimized_liveness(selected)
        .expect("zero-VReg structural functions must retain architectural liveness");
    assert_eq!(liveness.custody().function_count(), 0);
    assert_eq!(liveness.custody().structural_unit_function_count(), 2);
    assert!(liveness.liveness().plan().functions.is_empty());
    assert_eq!(
        liveness.liveness().plan().structural_unit_functions.len(),
        2
    );
    let live_caller = &liveness.liveness().plan().structural_unit_functions[0];
    assert!(live_caller.entry_definitions.is_empty());
    assert!(live_caller.operand_positions.is_empty());
    assert_eq!(live_caller.blocks[0].instructions.len(), 2);
    assert_eq!(
        live_caller.blocks[0].instructions[0].unit_uses,
        selected_call_uses
    );

    let ranges = stage_optimized_live_ranges(liveness)
        .expect("structural architectural flow must reach zero-VReg ranges");
    assert_eq!(ranges.custody().function_count(), 0);
    assert_eq!(ranges.custody().structural_unit_function_count(), 2);
    assert!(ranges.ranges().plan().functions.is_empty());
    assert_eq!(ranges.ranges().plan().structural_unit_functions.len(), 2);
    let range_caller = &ranges.ranges().plan().structural_unit_functions[0];
    assert!(range_caller.virtual_registers.is_empty());
    assert!(range_caller.tied_pairs.is_empty());
    assert!(range_caller.early_clobbers.is_empty());
    assert!(range_caller.interference.is_empty());
    assert!(!range_caller.architectural_units.is_empty());
    assert!(
        range_caller
            .architectural_units
            .iter()
            .any(|unit| !unit.actions.is_empty())
    );
    let mut corrupted = ranges.ranges().plan().clone();
    corrupted.structural_unit_functions[0].architectural_units[0]
        .actions
        .clear();
    assert!(
        validate_live_ranges(
            ranges.liveness_stage().selected_stage().selected(),
            ranges.liveness_stage().liveness(),
            corrupted,
        )
        .is_err()
    );

    let legality = stage_optimized_allocation_legality(ranges)
        .expect("zero-VReg structural functions must require no candidate homes");
    assert_eq!(legality.custody().function_count(), 0);
    assert_eq!(legality.custody().structural_unit_function_count(), 2);
    assert!(legality.legality().plan().functions.is_empty());
    assert_eq!(
        legality.legality().plan().structural_unit_functions.len(),
        2
    );
    assert!(
        legality
            .legality()
            .plan()
            .structural_unit_functions
            .iter()
            .all(|function| function.virtual_registers.is_empty())
    );
    let mut corrupted = legality.legality().plan().clone();
    corrupted.structural_unit_functions.swap(0, 1);
    let range_stage = legality.live_range_stage();
    let environment = range_stage
        .liveness_stage()
        .selected_stage()
        .register_environment();
    assert!(
        validate_allocation_legality(
            range_stage.ranges(),
            legality.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            corrupted,
        )
        .is_err()
    );

    let homes = stage_optimized_register_homes(legality)
        .expect("structural functions must receive exact empty home rosters");
    assert_eq!(homes.custody().function_count(), 0);
    assert_eq!(homes.custody().structural_unit_function_count(), 2);
    assert!(homes.homes().plan().functions.is_empty());
    assert_eq!(homes.homes().plan().structural_unit_functions.len(), 2);
    assert!(
        homes
            .homes()
            .plan()
            .structural_unit_functions
            .iter()
            .all(|function| function.assignments.is_empty())
    );
    assert_eq!(
        homes
            .post_allocation_manifest()
            .record()
            .statistics
            .functions,
        0
    );
    assert_eq!(
        homes
            .post_allocation_manifest()
            .record()
            .statistics
            .structural_unit_functions,
        2
    );
    let mut corrupted = homes.homes().plan().clone();
    corrupted.structural_unit_functions.swap(0, 1);
    let legality_stage = homes.legality_stage();
    let range_stage = legality_stage.live_range_stage();
    let environment = range_stage
        .liveness_stage()
        .selected_stage()
        .register_environment();
    assert!(
        validate_register_homes(
            legality_stage.legality(),
            range_stage.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            corrupted,
        )
        .is_err()
    );

    let post = stage_optimized_post_allocation_machine_plan(&homes)
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
    assert_eq!(post.machine().receipt().instruction_count(), 3);
    assert_eq!(post.machine().receipt().operand_count(), 0);
    let mut corrupted = post.machine().plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .unit_uses
        .clear();
    assert!(
        omega_machine_optimizer::validate_post_allocation_machine_plan(
            range_stage.liveness_stage().selected_stage().selected(),
            post.effects().effects(),
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

    let mut realization = stage_optimized_structural_unit_function_relative_realization(homes)
        .expect("structural Unit calls must reach owning function-relative custody");
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
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(Box::new(realization)),
    )
    .expect("structural Unit calls must retain typed unresolved fragment custody");
    assert!(fragments.fragments().functions.is_empty());
    assert_eq!(fragments.fragments().structural_unit_functions.len(), 2);
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
    assert_eq!(fragment_call.fixup.field_function_offset, 81);
    assert_eq!(fragment_call.fixup.next_instruction_function_offset, 85);
    assert_eq!(fragment_call.fixup.field_byte_width, 4);
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
    for unsupported in [5_u32, 7_u32, 10_u32] {
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
        .field_function_offset;
    fragments.fragments_mut().structural_unit_functions[0]
        .block
        .call
        .as_mut()
        .unwrap()
        .fixup
        .field_function_offset += 1;
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
        .field_function_offset = original_field_offset;
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
    for unsupported in [5_u32, 7_u32, 10_u32] {
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
