//! Widened-u8 exact-subtract selection through verified register and machine custody.

use crate::tests::*;

#[test]
fn widened_u8_exact_subtract_reaches_verified_register_and_machine_pipelines() {
    for (target, expected_homes, expected_alternative) in [
        (
            NativeTarget::linux_x64(),
            ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
            1,
        ),
        (
            NativeTarget::linux_arm64(),
            ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
            0,
        ),
    ] {
        let staged = staged_widened_u8_exact_subtract_conditional(target);
        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(function.virtual_registers.len(), 7);
        assert_eq!(staged.selected().receipt().instruction_count(), 10);
        assert_eq!(
            function
                .virtual_registers
                .iter()
                .map(|register| match register.origin {
                    VirtualRegisterOrigin::LegalizationTemporary {
                        temporary,
                        source_value,
                        ..
                    } => Some((temporary, source_value)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![
                None,
                Some((LegalizedTemporaryId(0), ValueId::new(5_106).unwrap())),
                Some((LegalizedTemporaryId(1), ValueId::new(5_107).unwrap())),
                None,
                Some((LegalizedTemporaryId(2), ValueId::new(5_110).unwrap())),
                Some((LegalizedTemporaryId(3), ValueId::new(5_111).unwrap())),
                None,
            ]
        );
        for (block, expected) in function.blocks[1..].iter().zip([
            (
                [IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)],
                [
                    OperationId::new(5_123).unwrap(),
                    OperationId::new(5_124).unwrap(),
                ],
                [
                    ValueId::new(5_106).unwrap(),
                    ValueId::new(5_107).unwrap(),
                    ValueId::new(5_108).unwrap(),
                    ValueId::new(5_109).unwrap(),
                ],
                ObligationId::new(5_131).unwrap(),
            ),
            (
                [IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)],
                [
                    OperationId::new(5_127).unwrap(),
                    OperationId::new(5_128).unwrap(),
                ],
                [
                    ValueId::new(5_110).unwrap(),
                    ValueId::new(5_111).unwrap(),
                    ValueId::new(5_112).unwrap(),
                    ValueId::new(5_113).unwrap(),
                ],
                ObligationId::new(5_132).unwrap(),
            ),
        ]) {
            assert_eq!(block.instructions.len(), 3);
            assert_eq!(
                block.instructions[0].kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: expected.0[0]
                }
            );
            assert_eq!(
                block.instructions[1].kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: expected.0[1]
                }
            );
            let subtract = &block.instructions[2];
            assert!(matches!(
                subtract.kind,
                SelectedInstructionKind::ExactSubtractI64 { obligation, .. }
                    if obligation == expected.3
            ));
            assert_eq!(
                subtract.constraint,
                staged.register_environment().selected_keys().subtract_i64
            );
            assert_eq!(
                subtract
                    .operands
                    .iter()
                    .map(|operand| (operand.virtual_register, operand.access))
                    .collect::<Vec<_>>(),
                vec![
                    (
                        block.instructions[0].operands[0].virtual_register,
                        RegisterOperandAccess::Use
                    ),
                    (
                        block.instructions[1].operands[0].virtual_register,
                        RegisterOperandAccess::Use
                    ),
                    (
                        subtract.operands[2].virtual_register,
                        RegisterOperandAccess::Def,
                    ),
                ]
            );
            assert_eq!(subtract.provenance.operations, expected.1);
            assert_eq!(subtract.provenance.values, expected.2);
            assert_eq!(subtract.provenance.obligations, vec![expected.3]);
            assert_eq!(
                subtract.provenance.fuel,
                expected
                    .1
                    .into_iter()
                    .map(|operation| FuelSettlement {
                        site: PsiProvenance::Operation(operation),
                        units: 1,
                    })
                    .collect::<Vec<_>>()
            );
            assert!(
                subtract
                    .operands
                    .iter()
                    .all(|operand| operand.fixed_view.is_none())
            );
            assert!(
                subtract
                    .operands
                    .iter()
                    .all(|operand| operand.tied_to.is_none())
            );
            if target.architecture == omega_target::Architecture::X86_64 {
                assert!(!subtract.clobbers.is_empty());
            } else {
                assert!(subtract.clobbers.is_empty());
            }
        }

        let selected_identity = staged.selected().receipt().identity();
        let mut swapped = staged.selected().plan().clone();
        swapped.functions[0].blocks[1].instructions[2]
            .operands
            .swap(0, 1);
        assert_ne!(
            selected_instruction_plan_identity(&swapped),
            selected_identity
        );
        assert!(matches!(
            validate_raw_selection(&staged, swapped),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let effects =
            analyze_machine_effects(staged.selected(), staged.register_environment()).unwrap();
        assert_eq!(effects.receipt().instruction_count(), 10);
        let subtracts = effects
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::ExactSubtractI64 { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(subtracts.len(), 2);
        for subtract in subtracts {
            assert_eq!(subtract.barrier, MachineBarrier::None);
            assert_eq!(
                subtract.alternatives.len(),
                if target.architecture == omega_target::Architecture::X86_64 {
                    4
                } else {
                    1
                }
            );
            assert_eq!(subtract.provenance.operations.len(), 2);
            assert_eq!(subtract.provenance.values.len(), 4);
            assert_eq!(subtract.provenance.obligations.len(), 1);
            assert_eq!(subtract.provenance.fuel.len(), 2);
        }

        let liveness = stage_optimized_liveness(staged).unwrap();
        assert_eq!(liveness.custody().instruction_count(), 10);
        let live_function = &liveness.liveness().plan().functions[0];
        for (block, registers) in live_function.blocks[1..]
            .iter()
            .zip([[1_u32, 2, 3], [4, 5, 6]])
        {
            assert_eq!(block.instructions.len(), 4);
            assert_eq!(
                block.instructions[2].virtual_uses,
                registers[..2]
                    .iter()
                    .copied()
                    .map(VirtualRegisterId)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                block.instructions[2].virtual_defs,
                vec![VirtualRegisterId(registers[2])]
            );
            assert_eq!(
                block.instructions[2].virtual_live_out,
                vec![VirtualRegisterId(registers[2])]
            );
        }

        let ranges = stage_optimized_live_ranges(liveness).unwrap();
        assert_eq!(ranges.custody().virtual_register_count(), 7);
        assert_eq!(
            ranges.ranges().plan().functions[0]
                .block_domains
                .iter()
                .map(|domain| (domain.block.0, domain.start.0, domain.end.0))
                .collect::<Vec<_>>(),
            vec![(0, 0, 4), (1, 4, 12), (2, 12, 20)]
        );
        assert_eq!(ranges.ranges().plan().functions[0].interference.len(), 2);

        let legality = stage_optimized_allocation_legality(ranges).unwrap();
        assert_eq!(legality.custody().entry_transition_count(), 0);
        assert_eq!(legality.custody().function_count(), 1);
        assert_eq!(legality.custody().structural_unit_function_count(), 0);
        let homes = stage_optimized_register_homes(legality).unwrap();
        assert_eq!(homes.custody().assignment_count(), 7);
        let selected_stage = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let model = selected_stage.register_environment().physical().model();
        assert_eq!(
            homes.homes().plan().functions[0]
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
        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert_eq!(post.custody().instruction_count(), 10);
        let post_subtracts = post
            .machine()
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                instruction.alternative.key.family
                    == omega_selected_instructions::MachineAlternativeFamily::ExactSubtractI64
            })
            .collect::<Vec<_>>();
        assert_eq!(post_subtracts.len(), 2);
        assert!(
            post_subtracts
                .iter()
                .all(|instruction| { instruction.alternative.key.variant == expected_alternative })
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_custody(&homes, &post).unwrap(),
            post.custody()
        );
    }
}
