use crate::tests::*;

#[test]
fn retained_selected_analysis_data_outlives_stages_and_requires_replay() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (liveness, ranges, liveness_commitment, ranges_commitment): (
            selected_instructions::LivenessPlan,
            selected_instructions::LiveRangePlan,
            selected_instructions::LivenessIdentity,
            selected_instructions::LiveRangeIdentity,
        ) = {
            let stage = stage_optimized_live_ranges(
                stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
            )
            .unwrap();
            (
                stage.liveness_stage().liveness().plan().clone(),
                stage.ranges().plan().clone(),
                stage.liveness_stage().liveness().receipt().identity(),
                stage.ranges().receipt().identity(),
            )
        };
        // Both raw plans remain usable after every producing stage was dropped.
        assert_eq!(liveness.functions[0].blocks.len(), 3);
        assert_eq!(ranges.functions[0].virtual_registers.len(), 2);
        assert_eq!(
            selected_instructions::liveness_identity(&liveness),
            liveness_commitment
        );
        assert_eq!(
            selected_instructions::live_range_identity(&ranges),
            ranges_commitment
        );

        let selected = staged_forwarded_conditional(target);
        let mut changed_liveness = liveness.clone();
        changed_liveness.functions[0].blocks[0]
            .virtual_live_in
            .clear();
        assert_ne!(
            selected_instructions::liveness_identity(&changed_liveness),
            liveness_commitment
        );
        assert!(validate_liveness(selected.selected(), changed_liveness).is_err());

        let admitted_liveness = validate_liveness(selected.selected(), liveness).unwrap();
        let mut changed_ranges = ranges.clone();
        changed_ranges.functions[0].interference.clear();
        assert_ne!(
            selected_instructions::live_range_identity(&changed_ranges),
            ranges_commitment
        );
        assert!(
            validate_live_ranges(selected.selected(), &admitted_liveness, changed_ranges).is_err()
        );
        let admitted_ranges =
            validate_live_ranges(selected.selected(), &admitted_liveness, ranges).unwrap();
        assert_eq!(admitted_ranges.receipt().identity(), ranges_commitment);
    }
}
#[test]
fn live_ranges_are_block_local_and_interference_is_cfg_exact() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap();
        let function = &staged.ranges().plan().functions[0];
        assert_eq!(
            function
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 6), (2, 6, 8)]
        );
        assert_eq!(function.virtual_registers.len(), 2);
        assert_eq!(
            function.virtual_registers[0].fragments,
            vec![LiveRangeFragment {
                block: selected_instructions::SelectedBlockId(0),
                start: LiveRangePoint(0),
                end: LiveRangePoint(1),
            }]
        );
        assert_eq!(
            function.virtual_registers[1]
                .fragments
                .iter()
                .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 5), (2, 6, 7)]
        );
        assert_eq!(
            function.virtual_registers[1]
                .edge_connectors
                .iter()
                .map(|edge| (edge.polarity_ordinal, edge.psi_edge, edge.target.0))
                .collect::<Vec<_>>(),
            vec![
                (0, EdgeId::new(4_011).unwrap(), 1),
                (1, EdgeId::new(4_012).unwrap(), 2),
            ]
        );
        assert_eq!(
            function.interference,
            vec![VirtualInterference {
                lower: VirtualRegisterId(0),
                higher: VirtualRegisterId(1),
            }]
        );
        assert_eq!(function.virtual_registers[0].fixed_constraints.len(), 1);
        assert!(matches!(
            function.virtual_registers[0].fixed_constraints[0].site,
            VirtualFixedConstraintSite::Entry
        ));
        assert_eq!(function.virtual_registers[1].fixed_constraints.len(), 3);
        assert!(matches!(
            function.virtual_registers[1].fixed_constraints[0].site,
            VirtualFixedConstraintSite::Entry
        ));
        assert!(
            function.virtual_registers[1].fixed_constraints[1..]
                .iter()
                .all(|constraint| matches!(
                    constraint.site,
                    VirtualFixedConstraintSite::Operand { .. }
                ))
        );
        assert_eq!(staged.custody().interference_count(), 1);
        assert_eq!(
            staged.custody().register_environment(),
            staged
                .liveness_stage()
                .selected_stage()
                .register_environment()
                .identity()
        );
        assert_eq!(
            staged.custody().ranges(),
            staged.ranges().receipt().identity()
        );
        assert_eq!(
            staged.custody().liveness(),
            staged.liveness_stage().liveness().receipt().identity()
        );

        let repeated = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap();
        assert_eq!(staged.ranges(), repeated.ranges());
        assert_eq!(staged.custody(), repeated.custody());
    }

    let constant = stage_optimized_live_ranges(
        stage_optimized_liveness(staged_conditional(NativeTarget::linux_x64())).unwrap(),
    )
    .unwrap();
    let function = &constant.ranges().plan().functions[0];
    assert_eq!(
        function
            .block_domains
            .iter()
            .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
            .collect::<Vec<_>>(),
        vec![(0, 0, 4), (1, 4, 8), (2, 8, 12)]
    );
    assert_eq!(
        function
            .virtual_registers
            .iter()
            .flat_map(|range| &range.fragments)
            .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
            .collect::<Vec<_>>(),
        vec![(0, 0, 1), (1, 5, 7), (2, 9, 11)]
    );
    assert!(function.interference.is_empty());
    assert!(
        function
            .virtual_registers
            .iter()
            .all(|range| range.edge_connectors.is_empty())
    );
}

#[test]
fn architectural_actions_do_not_inflate_semantic_unit_fragments() {
    for (target, instruction_pointer) in [
        (NativeTarget::linux_x64(), "rip"),
        (NativeTarget::linux_arm64(), "pc"),
    ] {
        let staged = stage_optimized_live_ranges(
            stage_optimized_liveness(staged_forwarded_conditional(target)).unwrap(),
        )
        .unwrap();
        let unit = named_units(staged.liveness_stage(), &[instruction_pointer])[0];
        let range = staged.ranges().plan().functions[0]
            .architectural_units
            .iter()
            .find(|range| range.unit == unit)
            .unwrap();
        assert_eq!(
            range
                .fragments
                .iter()
                .map(|fragment| (fragment.block.0, fragment.start.0, fragment.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 3)]
        );
        assert!(range.actions.iter().any(|action| {
            action.point == LiveRangePoint(3) && action.kind == ArchitecturalUnitActionKind::Def
        }));
    }
}

#[test]
fn independent_live_range_validation_rejects_corruption_and_detachment() {
    let staged =
        stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_x64())).unwrap();
    let valid = analyze_live_ranges(staged.selected_stage().selected(), staged.liveness()).unwrap();
    let identity = live_range_identity(valid.plan());

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].virtual_registers[1].fragments[0]
        .end
        .0 -= 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted.clone(),
        ),
        Err(LiveRangeError::VirtualRegisterMismatch { .. })
    ));
    assert_ne!(live_range_identity(&corrupted), identity);

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].virtual_registers[1].edge_connectors[0].polarity_ordinal = 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::NonCanonicalRows { .. })
            | Err(LiveRangeError::VirtualRegisterMismatch { .. })
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].interference.clear();
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::InterferenceMismatch { .. })
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].virtual_registers[1].fixed_constraints[0]
        .view
        .0 += 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::VirtualRegisterMismatch { .. })
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].architectural_units[0].actions[0]
        .point
        .0 += 1;
    assert!(matches!(
        validate_live_ranges(
            staged.selected_stage().selected(),
            staged.liveness(),
            corrupted,
        ),
        Err(LiveRangeError::ArchitecturalUnitMismatch { .. })
    ));

    let arm = stage_optimized_liveness(staged_forwarded_conditional(NativeTarget::linux_arm64()))
        .unwrap();
    let arm_ranges = analyze_live_ranges(arm.selected_stage().selected(), arm.liveness()).unwrap();
    assert!(matches!(
        validate_optimized_live_range_custody(&staged, &arm_ranges),
        Err(OptimizedLiveRangeCustodyError::Revalidation(
            LiveRangeError::RootMismatch
        ))
    ));
}
