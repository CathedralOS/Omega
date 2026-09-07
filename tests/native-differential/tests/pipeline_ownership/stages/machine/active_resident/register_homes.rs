//! Deterministic baseline and active-resident-rematerialized register homes.

use crate::tests::{
    IntegerValue, NativeTarget, OptimizationWorkBudget, PostAllocationSelectedTransformation,
    PressureRematerializationPolicy, RecoveryClassification, RecoveryClassificationPolicy,
    RecoveryVictimRole, SelectedInstructionKind, SpillChoicePolicy, ValueId, VirtualInterference,
    choose_spill_victims, classify_pressure_recovery, selected_lowering_budget,
    stage_optimized_active_resident_rematerialization, stage_optimized_allocation_legality,
    stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan, stage_optimized_register_homes,
    staged_active_resident_two_view_legality, staged_exact_add_conditional,
    validate_optimized_active_resident_rematerialization,
};

#[test]
fn exact_add_pressure_reaches_deterministic_homes_on_both_architectures() {
    for (target, expected_homes) in [
        (
            NativeTarget::linux_x64(),
            [
                "rdi", "rax", "rax", "rbx", "rax", "rax", "rax", "rbx", "rax", "rax",
            ],
        ),
        (
            NativeTarget::linux_arm64(),
            ["x0", "x0", "x0", "x1", "x0", "x0", "x0", "x1", "x0", "x0"],
        ),
    ] {
        let legality = stage_optimized_allocation_legality(
            stage_optimized_live_ranges(
                stage_optimized_liveness(staged_exact_add_conditional(target)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let ranges = legality.live_range_stage();
        let selected = ranges.liveness_stage().selected_stage();
        let environment = selected.register_environment();
        let choices = choose_spill_victims(
            legality.legality(),
            ranges.ranges(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
        )
        .unwrap();
        assert!(
            choices
                .plan()
                .functions
                .iter()
                .all(|function| function.choice.is_none())
        );
        let recovery = classify_pressure_recovery(
            selected.selected(),
            ranges.ranges(),
            legality.legality(),
            &choices,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            OptimizationWorkBudget::new(100, 100, 1_000, 100, 1).unwrap(),
        )
        .unwrap();
        assert!(
            recovery
                .plan()
                .functions
                .iter()
                .all(|function| function.classification.is_none())
        );
        assert_eq!(recovery.receipt().selected(), selected.custody().selected());
        assert_eq!(recovery.receipt().ranges(), ranges.custody().ranges());
        assert_eq!(recovery.receipt().legality(), legality.custody().legality());
        assert_eq!(
            recovery.receipt().spill_choices(),
            choices.receipt().identity()
        );
        let staged = stage_optimized_register_homes(legality).unwrap();
        let post = stage_optimized_post_allocation_machine_plan(&staged).unwrap();
        assert_eq!(
            post.machine().receipt().selected(),
            staged.custody().selected()
        );
        assert!(post.machine().plan().functions.iter().all(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .all(|instruction| instruction.alternative.key.variant == 0)
        }));
        let legality_stage = staged.legality_stage();
        let ranges_stage = legality_stage.live_range_stage();
        let liveness_stage = ranges_stage.liveness_stage();
        let liveness = &liveness_stage.liveness().plan().functions[0];
        let selected_function = &liveness_stage.selected_stage().selected().plan().functions[0];
        let adds = selected_function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::ExactAddI64 { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(adds.len(), 2);
        for (block, add) in liveness.blocks[1..].iter().zip(&adds) {
            let registers = add
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>();
            let live_add = block
                .instructions
                .iter()
                .find(|instruction| instruction.instruction == add.id)
                .unwrap();
            assert_eq!(block.instructions.len(), 5);
            assert_eq!(live_add.virtual_uses, registers[..2].to_vec());
            assert_eq!(live_add.virtual_defs, vec![registers[2]]);
            assert_eq!(live_add.virtual_live_out, vec![registers[2]]);
        }

        let ranges = &ranges_stage.ranges().plan().functions[0];
        assert_eq!(
            ranges
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 6), (1, 6, 16), (2, 16, 26)]
        );
        assert_eq!(
            ranges.interference,
            vec![
                VirtualInterference {
                    lower: adds[0].operands[0].virtual_register,
                    higher: adds[0].operands[1].virtual_register,
                },
                VirtualInterference {
                    lower: adds[1].operands[0].virtual_register,
                    higher: adds[1].operands[1].virtual_register,
                },
            ]
        );
        assert!(
            ranges
                .virtual_registers
                .iter()
                .all(|register| register.edge_connectors.is_empty())
        );
        assert_eq!(legality_stage.custody().entry_transition_count(), 0);

        let environment = liveness_stage.selected_stage().register_environment();
        let model = environment.physical().model();
        let homes = &staged.homes().plan().functions[0];
        assert_eq!(homes.assignments.len(), 10);
        assert_eq!(
            homes
                .assignments
                .iter()
                .map(|assignment| {
                    model
                        .views
                        .iter()
                        .find(|view| view.id == assignment.view)
                        .unwrap()
                        .name
                        .as_str()
                })
                .collect::<Vec<_>>(),
            expected_homes
        );
        let home = |register| {
            homes
                .assignments
                .iter()
                .find(|assignment| assignment.virtual_register == register)
                .unwrap()
                .view
        };
        assert_eq!(
            home(adds[0].operands[0].virtual_register),
            home(adds[1].operands[0].virtual_register)
        );
        assert_eq!(
            home(adds[0].operands[1].virtual_register),
            home(adds[1].operands[1].virtual_register)
        );
        assert_ne!(
            home(adds[0].operands[0].virtual_register),
            home(adds[0].operands[1].virtual_register)
        );
    }
}

#[test]
fn active_resident_multi_use_rematerialization_reaches_fresh_homes_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = staged_active_resident_two_view_legality(target);

        let source_selected = source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .selected()
            .plan()
            .clone();
        let source_resident = source_selected.functions[0].blocks[1].instructions[0].clone();
        let resident_register = source_resident.operands[0].virtual_register;
        let incoming_register = source_selected.functions[0].blocks[1]
            .instructions
            .iter()
            .find(|instruction| {
                instruction
                    .provenance
                    .values
                    .contains(&ValueId::new(5_208).unwrap())
            })
            .unwrap()
            .operands[0]
            .virtual_register;
        assert!(matches!(
            source_resident.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(3)
            }
        ));

        let staged = stage_optimized_active_resident_rematerialization(
            source,
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        )
        .unwrap();
        assert_eq!(
            validate_optimized_active_resident_rematerialization(&staged).unwrap(),
            staged.custody()
        );
        let choice = staged.choices().plan().functions[0]
            .choice
            .as_ref()
            .unwrap();
        assert_eq!(choice.incoming, incoming_register);
        assert_eq!(choice.selected_victim, resident_register);
        let classification = staged.classifications().plan().functions[0]
            .classification
            .as_ref()
            .unwrap();
        assert_eq!(classification.victim, resident_register);
        assert!(matches!(
            classification.role,
            RecoveryVictimRole::ActiveResident { .. }
        ));
        let RecoveryClassification::ImmediateU64RematerializationCandidate {
            defining_instruction,
            source_value,
            value,
            provenance,
            future_uses,
        } = &classification.classification
        else {
            panic!("active resident must retain literal eligibility")
        };
        assert_eq!(*defining_instruction, source_resident.id);
        assert_eq!(*source_value, ValueId::new(5_206).unwrap());
        assert_eq!(*value, IntegerValue::Unsigned(3));
        assert_eq!(provenance, &source_resident.provenance);
        assert_eq!(future_uses.len(), 2);

        let action = staged.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap();
        assert_eq!(action.victim, resident_register);
        assert_eq!(action.original_materialize, source_resident.id);
        assert_eq!(action.rewrites.len(), 2);
        assert_eq!(
            staged.rematerialization().receipt().rewritten_use_count(),
            2
        );
        assert_eq!(staged.rematerialization().receipt().applied_count(), 1);
        let transformed = staged.rematerialization().transformed();
        assert_eq!(
            transformed.functions[0].blocks[1].instructions[0],
            source_resident
        );
        let fresh = transformed.functions[0].blocks[1]
            .instructions
            .iter()
            .find(|instruction| instruction.id == action.fresh_materialize)
            .unwrap();
        assert!(fresh.provenance.operations.is_empty());
        assert!(fresh.provenance.edges.is_empty());
        assert!(fresh.provenance.obligations.is_empty());
        assert!(fresh.provenance.fuel.is_empty());
        assert_eq!(fresh.provenance.values, vec![ValueId::new(5_206).unwrap()]);
        let rewritten_uses = transformed.functions[0].blocks[1]
            .instructions
            .iter()
            .flat_map(|instruction| &instruction.operands)
            .filter(|operand| operand.virtual_register == action.result_virtual_register)
            .count();
        assert_eq!(rewritten_uses, 3);
        assert_ne!(
            staged.liveness().receipt().identity(),
            staged.source().custody().liveness()
        );
        assert_ne!(
            staged.ranges().receipt().identity(),
            staged.source().custody().ranges()
        );
        assert_ne!(
            staged.legality().receipt().identity(),
            staged.source().custody().legality()
        );
        assert_eq!(staged.legality().receipt().entry_transition_count(), 0);
        assert_eq!(
            staged.homes().receipt().ranges(),
            staged.ranges().receipt().identity()
        );
        assert_eq!(
            staged.homes().receipt().legality(),
            staged.legality().receipt().identity()
        );
        assert_eq!(staged.homes().receipt().assignment_count(), 12);
        assert_eq!(
            staged
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            vec![
                PostAllocationSelectedTransformation::PressureRematerialization(
                    staged.rematerialization().receipt().identity()
                )
            ]
        );
        assert_eq!(
            staged.post_allocation_manifest().record().selected,
            staged.rematerialization().receipt().transformed_selected()
        );
    }
}
