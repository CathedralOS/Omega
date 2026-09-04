//! Widened-u8 exact-add selection, effects, register allocation, and machine planning.

use crate::tests::*;

#[test]

fn widened_u8_exact_add_reaches_selected_effect_and_register_pipelines_on_both_architectures() {
    for (target, expected_homes) in [
        (
            NativeTarget::linux_x64(),
            ["rdi", "rax", "rbx", "rax", "rax", "rbx", "rax"],
        ),
        (
            NativeTarget::linux_arm64(),
            ["x0", "x0", "x1", "x0", "x0", "x1", "x0"],
        ),
    ] {
        let staged = staged_widened_u8_exact_add_conditional(target);
        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(function.virtual_registers.len(), 7);
        assert_eq!(staged.selected().receipt().instruction_count(), 10);
        assert!(
            function.blocks[0].instructions[0]
                .provenance
                .fuel
                .is_empty()
        );
        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } = &function.blocks[0].terminator
        else {
            panic!("selected entry must branch")
        };
        assert!(instruction.provenance.fuel.is_empty());
        assert_eq!(
            when_nonzero.fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_141).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            when_zero.fuel,
            vec![FuelSettlement {
                site: PsiProvenance::Edge(EdgeId::new(5_142).unwrap()),
                units: 1,
            }]
        );
        assert_eq!(
            function
                .virtual_registers
                .iter()
                .map(|register| match register.origin {
                    VirtualRegisterOrigin::LegalizationTemporary { temporary, .. } =>
                        Some(temporary),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![
                None,
                Some(LegalizedTemporaryId(0)),
                Some(LegalizedTemporaryId(1)),
                None,
                Some(LegalizedTemporaryId(2)),
                Some(LegalizedTemporaryId(3)),
                None,
            ]
        );
        for (block, expected) in function.blocks[1..].iter().zip([
            (
                [IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)],
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
                [IntegerValue::Unsigned(254), IntegerValue::Unsigned(1)],
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
            for (materialize, expected_value) in block.instructions[..2].iter().zip(expected.0) {
                assert_eq!(
                    materialize.kind,
                    SelectedInstructionKind::MaterializeI64 {
                        value: expected_value,
                    }
                );
            }
            let add = &block.instructions[2];
            assert!(matches!(
                add.kind,
                SelectedInstructionKind::ExactAddI64 { obligation, .. }
                    if obligation == expected.3
            ));
            assert_eq!(
                add.constraint,
                staged.register_environment().selected_keys().add_i64
            );
            assert_eq!(
                add.operands
                    .iter()
                    .map(|operand| operand.access)
                    .collect::<Vec<_>>(),
                vec![
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Def,
                ]
            );
            assert_eq!(add.provenance.operations, expected.1);
            assert_eq!(add.provenance.values, expected.2);
            assert_eq!(add.provenance.obligations, vec![expected.3]);
            assert_eq!(
                add.provenance.fuel,
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
                add.operands
                    .iter()
                    .all(|operand| operand.fixed_view.is_none())
            );
            assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
            assert!(add.implicit_uses.is_empty());
            assert!(add.implicit_defs.is_empty());
            assert!(add.clobbers.is_empty());
            let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
                panic!("selected leaf must return")
            };
            assert_eq!(instruction.provenance.values, vec![expected.2[3]]);
            assert_eq!(instruction.provenance.fuel.len(), 1);
        }
        for (register, expected_source, expected_site) in [
            (
                &function.virtual_registers[3],
                ValueId::new(5_109).unwrap(),
                ValueDefinitionSite::Node {
                    block: BlockId::new(5_103).unwrap(),
                    node: 3,
                },
            ),
            (
                &function.virtual_registers[6],
                ValueId::new(5_113).unwrap(),
                ValueDefinitionSite::Node {
                    block: BlockId::new(5_104).unwrap(),
                    node: 3,
                },
            ),
        ] {
            assert_eq!(register.definition_site, expected_site);
            assert!(matches!(
                register.origin,
                VirtualRegisterOrigin::InstructionResult { source_value, .. }
                    if source_value == expected_source
            ));
        }

        let selected_identity = staged.selected().receipt().identity();
        let mut corrupted = staged.selected().plan().clone();
        let VirtualRegisterOrigin::LegalizationTemporary { temporary, .. } =
            &mut corrupted.functions[0].virtual_registers[1].origin
        else {
            panic!("first widened operand must retain its legal temporary")
        };
        temporary.0 += 10;
        assert_ne!(
            selected_instruction_plan_identity(&corrupted),
            selected_identity
        );
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::VirtualRegisterProjectionMismatch { .. })
        ));

        let effects =
            analyze_machine_effects(staged.selected(), staged.register_environment()).unwrap();
        assert_eq!(effects.receipt().instruction_count(), 10);
        let adds = effects
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::ExactAddI64 { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(adds.len(), 2);
        for add in adds {
            assert_eq!(add.barrier, MachineBarrier::None);
            assert_eq!(add.alternatives.len(), 1);
            assert!(add.unit_clobbers.is_empty());
            assert_eq!(add.provenance.operations.len(), 2);
            assert_eq!(add.provenance.values.len(), 4);
            assert_eq!(add.provenance.obligations.len(), 1);
            assert_eq!(add.provenance.fuel.len(), 2);
        }

        let homes = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(stage_optimized_liveness(staged).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let selected_stage = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let model = selected_stage.register_environment().physical().model();
        assert_eq!(homes.custody().assignment_count(), 7);
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
    }
}
