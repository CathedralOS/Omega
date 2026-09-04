use crate::tests::*;

#[test]
fn zero_vreg_unit_return_reaches_replayed_homes_and_machine_custody() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
    ] {
        let (semantic, proof, selected) = staged_unit_return(target);
        let selected_function = &selected.selected().plan().functions[0];
        assert!(selected_function.virtual_registers.is_empty());
        assert_eq!(selected_function.blocks.len(), 1);
        assert!(selected_function.blocks[0].instructions.is_empty());
        let SelectedTerminator::Return { instruction, .. } =
            &selected_function.blocks[0].terminator
        else {
            panic!("Unit selection must end in the generic return terminator");
        };
        assert_eq!(instruction.kind, SelectedInstructionKind::ReturnUnit);
        assert!(instruction.operands.is_empty());

        let raw_liveness = analyze_liveness(selected.selected()).unwrap();
        let mut corrupted_liveness = raw_liveness.plan().clone();
        corrupted_liveness.functions[0].blocks[0]
            .unit_live_in
            .clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted_liveness),
            Err(LivenessError::BlockMismatch {
                function: 0,
                block: 0
            })
        ));

        let liveness = stage_optimized_liveness(selected).unwrap();
        assert_eq!(liveness.custody().function_count(), 1);
        assert_eq!(liveness.custody().structural_unit_function_count(), 0);
        assert_eq!(liveness.custody().block_count(), 1);
        assert_eq!(liveness.custody().virtual_register_count(), 0);
        assert_eq!(liveness.custody().instruction_count(), 1);
        assert_eq!(liveness.custody().successor_count(), 0);
        let live_function = &liveness.liveness().plan().functions[0];
        assert!(live_function.entry_definitions.is_empty());
        assert!(live_function.operand_positions.is_empty());
        assert!(live_function.blocks[0].virtual_live_in.is_empty());
        assert!(live_function.blocks[0].virtual_live_out.is_empty());
        assert!(!live_function.blocks[0].unit_live_in.is_empty());
        assert!(live_function.blocks[0].unit_live_out.is_empty());

        let raw_ranges =
            analyze_live_ranges(liveness.selected_stage().selected(), liveness.liveness()).unwrap();
        let mut corrupted_ranges = raw_ranges.plan().clone();
        corrupted_ranges.functions[0].architectural_units.pop();
        assert!(matches!(
            validate_live_ranges(
                liveness.selected_stage().selected(),
                liveness.liveness(),
                corrupted_ranges,
            ),
            Err(LiveRangeError::ArchitecturalUnitMismatch { function: 0, .. })
        ));

        let ranges = stage_optimized_live_ranges(liveness).unwrap();
        assert_eq!(ranges.custody().function_count(), 1);
        assert_eq!(ranges.custody().structural_unit_function_count(), 0);
        assert_eq!(ranges.custody().block_count(), 1);
        assert_eq!(ranges.custody().virtual_register_count(), 0);
        assert_eq!(ranges.custody().virtual_occurrence_count(), 0);
        assert_eq!(ranges.custody().fixed_constraint_count(), 0);
        assert_eq!(ranges.custody().virtual_fragment_count(), 0);
        assert_eq!(ranges.custody().interference_count(), 0);
        assert!(ranges.custody().architectural_unit_count() > 0);
        assert_eq!(
            ranges.ranges().plan().functions[0]
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 2)]
        );

        let legality = stage_optimized_allocation_legality(ranges).unwrap();
        assert_eq!(legality.custody().function_count(), 1);
        assert_eq!(legality.custody().structural_unit_function_count(), 0);
        assert_eq!(legality.custody().virtual_register_count(), 0);
        assert_eq!(legality.custody().point_count(), 0);
        assert_eq!(legality.custody().candidate_count(), 0);
        assert_eq!(legality.custody().entry_transition_count(), 0);
        let range_stage = legality.live_range_stage();
        let environment = range_stage
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let mut corrupted_legality = legality.legality().plan().clone();
        corrupted_legality.functions[0].machine = MachineId::new(3_599).unwrap();
        assert!(matches!(
            validate_allocation_legality(
                range_stage.ranges(),
                legality.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted_legality,
            ),
            Err(AllocationLegalityError::FunctionMismatch { function: 0 })
        ));

        let homes = stage_optimized_register_homes(legality).unwrap();
        assert_eq!(homes.custody().function_count(), 1);
        assert_eq!(homes.custody().structural_unit_function_count(), 0);
        assert_eq!(homes.custody().assignment_count(), 0);
        assert!(homes.homes().plan().functions[0].assignments.is_empty());
        assert!(
            homes
                .post_allocation_manifest()
                .record()
                .selected_transformations
                .is_empty()
        );
        let legality_stage = homes.legality_stage();
        let range_stage = legality_stage.live_range_stage();
        let environment = range_stage
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let mut corrupted_homes = homes.homes().plan().clone();
        corrupted_homes.functions[0].machine = MachineId::new(3_599).unwrap();
        assert!(matches!(
            validate_register_homes(
                legality_stage.legality(),
                range_stage.ranges(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                corrupted_homes,
            ),
            Err(RegisterHomeError::FunctionMismatch { function: 0 })
        ));

        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert_eq!(post.custody().instruction_count(), 1);
        assert_eq!(post.machine().plan().functions.len(), 1);
        let post_instruction = &post.machine().plan().functions[0].blocks[0].instructions[0];
        assert_eq!(
            post_instruction.alternative.key.family,
            omega_selected_instructions::MachineAlternativeFamily::ReturnUnit
        );
        assert!(post_instruction.operands.is_empty());
        let mut corrupted_post = post.machine().plan().clone();
        corrupted_post.functions[0].machine = MachineId::new(3_599).unwrap();
        let selected_stage = range_stage.liveness_stage().selected_stage();
        assert!(
            omega_machine_optimizer::validate_post_allocation_machine_plan(
                selected_stage.selected(),
                post.effects(),
                range_stage.ranges(),
                legality_stage.legality(),
                homes.homes(),
                homes.post_allocation_manifest(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                corrupted_post,
            )
            .is_err()
        );

        let mut realization = stage_optimized_unit_function_relative_realization(homes).unwrap();
        assert_eq!(realization.manifest().record().statistics.functions, 1);
        assert_eq!(realization.manifest().record().statistics.blocks, 1);
        assert_eq!(realization.manifest().record().statistics.instructions, 1);
        assert_eq!(
            realization.manifest().record().statistics.bytes,
            match target.architecture {
                omega_target::Architecture::X86_64 => 1,
                omega_target::Architecture::Aarch64 => 4,
            }
        );
        let [function] = realization.exit_contract().contract().functions.as_slice() else {
            panic!("Unit exit contract must retain one function");
        };
        let [returned] = function.returns.as_slice() else {
            panic!("Unit exit contract must retain one return");
        };
        assert!(matches!(
            returned.value,
            whole_function_exit_contract::WholeFunctionReturnValueEvidence::UnitV1
        ));
        let receipt = validate_optimized_unit_function_relative_realization(&realization).unwrap();
        assert_eq!(receipt, *realization.custody());
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(
                &realization.manifest().record().encode()
            ),
            Ok(realization.manifest().record().clone())
        );

        realization.exit_contract_mut().contract_mut().functions[0].returns[0].value =
            whole_function_exit_contract::WholeFunctionReturnValueEvidence::ScalarI64V1 {
                virtual_register: VirtualRegisterId(0),
                view: RegisterViewId(0),
                units: Vec::new(),
            };
        assert!(matches!(
            validate_optimized_unit_function_relative_realization(&realization),
            Err(OptimizedUnitFunctionRelativeRealizationError::Exit(
                WholeFunctionExitContractError::ArtifactMismatch
            ))
        ));
        realization.exit_contract_mut().contract_mut().functions[0].returns[0].value =
            WholeFunctionReturnValueEvidence::UnitV1;
        validate_optimized_unit_function_relative_realization(&realization).unwrap();

        let fragments = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(Box::new(realization)),
        )
        .unwrap();
        assert_eq!(
            fragments.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::UnitBaselineV1
        );
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&fragments.manifest().record().encode()),
            Ok(fragments.manifest().record().clone())
        );
        assert_eq!(fragments.fragments().functions.len(), 1);
        assert_eq!(fragments.fragments().functions[0].blocks.len(), 1);
        let emitted_bytes = fragments.fragments().functions[0].bytes.clone();
        assert_eq!(
            emitted_bytes.as_slice(),
            match target.architecture {
                omega_target::Architecture::X86_64 => &[0xc3][..],
                omega_target::Architecture::Aarch64 => &[0xc0, 0x03, 0x5f, 0xd6][..],
            }
        );

        let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
        assert_eq!(
            text.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::UnitBaselineV1
        );
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&text.manifest().record().encode()),
            Ok(text.manifest().record().clone())
        );
        assert_eq!(text.text_section().bytes, emitted_bytes);
        let object = stage_optimized_relocation_free_object_container(text).unwrap();
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap();
        assert_eq!(
            artifact.artifact().semantic_entry,
            MachineId::new(3_501).unwrap()
        );
        assert_eq!(
            artifact.artifact().statistics.text_bytes,
            emitted_bytes.len() as u64
        );
        assert_eq!(artifact.artifact().statistics.relocation_records, 0);
    }
}
