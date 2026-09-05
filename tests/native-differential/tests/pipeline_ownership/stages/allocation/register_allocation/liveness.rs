use crate::tests::*;
#[test]
fn selected_liveness_is_exact_on_both_architectures() {
    for (target, before_compare, after_compare, after_branch) in [
        (
            NativeTarget::linux_x64(),
            vec!["rip", "rsp"],
            vec!["rflags", "rip", "rsp"],
            vec!["rsp"],
        ),
        (
            NativeTarget::linux_arm64(),
            vec!["pc", "sp", "x30"],
            vec!["nzcv", "pc", "sp", "x30"],
            vec!["sp", "x30"],
        ),
    ] {
        let staged = stage_optimized_liveness(staged_conditional(target)).unwrap();
        let plan = staged.liveness().plan();
        let function = &plan.functions[0];
        assert_eq!(function.entry_definitions.len(), 1);
        assert_eq!(
            function.entry_definitions[0].virtual_register,
            VirtualRegisterId(0)
        );
        assert!(function.entry_definitions[0].fixed_view.is_some());
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .map(|instruction| instruction.position.0)
                .collect::<Vec<_>>(),
            (0..6).collect::<Vec<_>>()
        );

        let entry = &function.blocks[0];
        assert_eq!(entry.virtual_live_in, vec![VirtualRegisterId(0)]);
        assert!(entry.virtual_live_out.is_empty());
        assert_eq!(
            entry.instructions[0].virtual_uses,
            vec![VirtualRegisterId(0)]
        );
        assert!(entry.instructions[0].virtual_defs.is_empty());
        assert_eq!(
            entry.instructions[0].unit_live_in,
            named_units(&staged, &before_compare)
        );
        assert_eq!(
            entry.instructions[0].unit_live_out,
            named_units(&staged, &after_compare)
        );
        assert_eq!(
            entry.instructions[1].unit_live_in,
            entry.instructions[0].unit_live_out
        );
        assert_eq!(
            entry.instructions[1].unit_live_out,
            named_units(&staged, &after_branch)
        );
        assert_eq!(entry.successors.len(), 2);
        assert_eq!(entry.successors[0].polarity_ordinal, 0);
        assert_eq!(entry.successors[1].polarity_ordinal, 1);
        for successor in &entry.successors {
            let target_block = &function.blocks[successor.target.0 as usize];
            assert_eq!(successor.virtual_live, target_block.virtual_live_in);
            assert_eq!(successor.unit_live, target_block.unit_live_in);
        }

        for (block, register) in function.blocks[1..]
            .iter()
            .zip([VirtualRegisterId(1), VirtualRegisterId(2)])
        {
            assert!(block.virtual_live_in.is_empty());
            assert!(block.virtual_live_out.is_empty());
            assert_eq!(block.instructions[0].virtual_defs, vec![register]);
            assert_eq!(block.instructions[0].virtual_live_out, vec![register]);
            assert_eq!(block.instructions[1].virtual_uses, vec![register]);
            assert_eq!(block.instructions[1].virtual_live_in, vec![register]);
            assert!(block.instructions[1].virtual_live_out.is_empty());
        }
        assert_eq!(staged.custody().function_count(), 1);
        assert_eq!(staged.custody().block_count(), 3);
        assert_eq!(staged.custody().virtual_register_count(), 3);
        assert_eq!(staged.custody().instruction_count(), 6);
        assert_eq!(staged.custody().successor_count(), 2);
        assert_eq!(
            staged.custody().register_environment(),
            staged.selected_stage().register_environment().identity()
        );
        assert_eq!(
            staged.custody().liveness(),
            staged.liveness().receipt().identity()
        );
        assert_eq!(
            staged.custody().selected(),
            staged.selected_stage().selected().receipt().identity()
        );
    }
}

#[test]
fn forwarded_parameter_conditional_retains_cross_edge_liveness() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_forwarded_conditional(target);
        assert_eq!(
            selected.legalized().plan().functions[0].recipe,
            LegalizationRecipe::ReturnU64EntryParameterConditionalV1
        );
        let selected_plan = selected.selected().plan();
        assert_eq!(selected_plan.functions[0].virtual_registers.len(), 2);
        assert_eq!(
            selected_plan.functions[0]
                .blocks
                .iter()
                .map(|block| block.instructions.len() + 1)
                .sum::<usize>(),
            4
        );
        assert!(
            selected_plan.functions[0].virtual_registers[0]
                .entry_fixed_view
                .is_some()
        );
        assert!(
            selected_plan.functions[0].virtual_registers[1]
                .entry_fixed_view
                .is_some()
        );

        let staged = stage_optimized_liveness(selected).unwrap();
        let function = &staged.liveness().plan().functions[0];
        assert_eq!(
            function.blocks[0].virtual_live_in,
            vec![VirtualRegisterId(0), VirtualRegisterId(1)]
        );
        assert_eq!(
            function.blocks[0].virtual_live_out,
            vec![VirtualRegisterId(1)]
        );
        for successor in &function.blocks[0].successors {
            assert_eq!(successor.virtual_live, vec![VirtualRegisterId(1)]);
        }
        for block in &function.blocks[1..] {
            assert_eq!(block.virtual_live_in, vec![VirtualRegisterId(1)]);
            assert!(block.virtual_live_out.is_empty());
            assert_eq!(
                block.instructions[0].virtual_uses,
                vec![VirtualRegisterId(1)]
            );
            assert!(block.instructions[0].virtual_live_out.is_empty());
            assert!(block.instructions[0].unit_live_out.is_empty());
        }
    }
}

#[test]
fn selected_liveness_is_deterministic_and_identity_binds_every_domain() {
    let first = stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
    let second = stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap();
    assert_eq!(first.liveness(), second.liveness());
    assert_eq!(first.custody(), second.custody());

    let original = first.liveness().plan();
    let identity = liveness_identity(original);
    let mut mutations = Vec::new();
    let mut changed = original.clone();
    changed.selected = selected_instructions::SelectedInstructionPlanIdentity::from_canonical_bytes(
        b"changed-selected",
    );
    mutations.push(changed);
    let mut changed = original.clone();
    changed.target = NativeTarget::windows_x64();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.fuel_schedule = semantic_vocabulary::FuelScheduleIdentity::new(
        original.fuel_schedule.marker().checked_add(1).unwrap(),
    )
    .unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.optimization_unit =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"changed");
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].machine = MachineId::new(8_101).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_definitions[0].fixed_view = None;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_definitions[0].virtual_register = VirtualRegisterId(8);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].entry_definitions[0].class.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].position.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].instruction.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].operand += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].virtual_register.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].access = RegisterOperandAccess::Def;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].class.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].fixed_view =
        changed.functions[0].entry_definitions[0].fixed_view;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].tied_to = Some(0);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].operand_positions[0].early_clobber = true;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].block.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].source_block = BlockId::new(8_103).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .virtual_live_in
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .unit_live_in
        .push(RegisterUnitId(u16::MAX));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .virtual_live_out
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0]
        .unit_live_out
        .push(RegisterUnitId(u16::MAX));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0].position.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0].instruction.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .virtual_uses
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .virtual_defs
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .virtual_live_in
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[1].instructions[0]
        .virtual_live_out
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[1]
        .unit_uses
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_defs
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_clobbers
        .push(RegisterUnitId(u16::MAX));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_live_in
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .unit_live_out
        .clear();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].polarity_ordinal = 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].psi_edge = EdgeId::new(8_102).unwrap();
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].terminator.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0].target.0 += 1;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0]
        .virtual_live
        .push(VirtualRegisterId(9));
    mutations.push(changed);
    let mut changed = original.clone();
    changed.functions[0].blocks[0].successors[0]
        .unit_live
        .clear();
    mutations.push(changed);
    for mutation in mutations {
        assert_ne!(liveness_identity(&mutation), identity);
    }
}

#[test]
fn independent_liveness_validator_rejects_raw_transfer_and_path_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_conditional(target);
        let valid = analyze_liveness(selected.selected()).unwrap();

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[0].virtual_live_in.clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::BlockMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[0].instructions[0]
            .unit_live_out
            .clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::TransferMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[1].instructions[1]
            .virtual_live_in
            .clear();
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::TransferMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[0].successors.swap(0, 1);
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::SuccessorMismatch { .. })
        ));

        let mut corrupted = valid.plan().clone();
        corrupted.functions[0].blocks[2].instructions[0].position.0 = 99;
        assert!(matches!(
            validate_liveness(selected.selected(), corrupted),
            Err(LivenessError::NonDensePositions { .. })
        ));
    }
}

#[test]
fn liveness_custody_rejects_a_detached_same_shape_target() {
    let x86 = staged_conditional(NativeTarget::linux_x64());
    let arm = staged_conditional(NativeTarget::linux_arm64());
    let arm_liveness = analyze_liveness(arm.selected()).unwrap();
    assert!(matches!(
        validate_optimized_liveness_custody(&x86, &arm_liveness),
        Err(OptimizedLivenessCustodyError::Revalidation(
            LivenessError::RootMismatch
        ))
    ));
}
