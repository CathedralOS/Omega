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

        let effects = stage_optimized_machine_effects(&staged).unwrap();
        assert_eq!(effects.custody().instruction_count(), 10);
        let adds = effects
            .effects()
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

        let effects = stage_optimized_machine_effects(&staged).unwrap();
        assert_eq!(effects.custody().instruction_count(), 10);
        let subtracts = effects
            .effects()
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

#[test]
fn legalization_replay_rejects_foreign_proof_fact_and_leaf_operation_custody() {
    let staged = staged_exact_add_conditional(NativeTarget::linux_x64());
    let original = staged.legalized().plan();
    let validate = |plan| {
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            plan,
        )
    };

    let mut corrupted = original.clone();
    let false_fact = match corrupted.functions[0].when_false.value {
        omega_legalized_operations::LegalizedLeafValue::ExactAdd { accepted_fact, .. } => {
            accepted_fact
        }
        _ => panic!("exact-add fixture must retain its admitted fact"),
    };
    let omega_legalized_operations::LegalizedLeafValue::ExactAdd { accepted_fact, .. } =
        &mut corrupted.functions[0].when_true.value
    else {
        panic!("exact-add fixture must retain its admitted fact")
    };
    *accepted_fact = false_fact;
    assert_eq!(
        validate(corrupted),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );

    let mut corrupted = original.clone();
    let omega_legalized_operations::LegalizedLeafValue::ExactAdd { left, right, .. } =
        &mut corrupted.functions[0].when_true.value
    else {
        panic!("exact-add fixture must retain its inputs")
    };
    std::mem::swap(&mut left.constant_operation, &mut right.constant_operation);
    assert_eq!(
        validate(corrupted),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}

#[test]
fn exact_add_selection_retains_proof_policy_and_target_constraints() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_exact_add_conditional(target);
        assert_eq!(
            staged.legalized().plan().functions[0].recipe,
            LegalizationRecipe::ReturnU64ExactAddImmediateConditionalV1
        );
        let plan = staged.selected().plan();
        let function = &plan.functions[0];
        assert_eq!(function.virtual_registers.len(), 7);
        assert_eq!(staged.selected().receipt().instruction_count(), 10);
        let accepted = &staged
            .optimized_target()
            .optimized()
            .unit()
            .accepted_obligation_facts;
        assert_eq!(accepted.len(), 2);
        for (block, expected_obligation) in function.blocks[1..].iter().zip([
            ObligationId::new(5_031).unwrap(),
            ObligationId::new(5_032).unwrap(),
        ]) {
            assert_eq!(block.instructions.len(), 3);
            let add = &block.instructions[2];
            let SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            } = add.kind
            else {
                panic!("leaf arithmetic must retain exact-add semantics")
            };
            assert_eq!(obligation, expected_obligation);
            let fact = accepted
                .iter()
                .find(|fact| fact.identity == accepted_fact)
                .expect("selected fact must remain verifier-owned");
            assert_eq!(fact.operation, add.provenance.operations[0]);
            assert_eq!(fact.obligation, obligation);
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
            assert!(
                add.operands
                    .iter()
                    .all(|operand| operand.fixed_view.is_none())
            );
            assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
            assert!(add.implicit_uses.is_empty());
            assert!(add.implicit_defs.is_empty());
            assert!(add.clobbers.is_empty());
            assert_eq!(add.provenance.operations.len(), 1);
            assert_eq!(add.provenance.values.len(), 3);
            assert_eq!(add.provenance.obligations, vec![obligation]);
            assert_eq!(add.provenance.fuel.len(), 1);
        }

        let original_identity = staged.selected().receipt().identity();
        let mut corrupted = plan.clone();
        let SelectedInstructionKind::ExactAddI64 { obligation, .. } =
            &mut corrupted.functions[0].blocks[1].instructions[2].kind
        else {
            unreachable!()
        };
        *obligation = ObligationId::new(9_501).unwrap();
        assert_ne!(
            selected_instruction_plan_identity(&corrupted),
            original_identity
        );
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        let false_fact = match corrupted.functions[0].blocks[2].instructions[2].kind {
            SelectedInstructionKind::ExactAddI64 { accepted_fact, .. } => accepted_fact,
            _ => unreachable!(),
        };
        let SelectedInstructionKind::ExactAddI64 { accepted_fact, .. } =
            &mut corrupted.functions[0].blocks[1].instructions[2].kind
        else {
            unreachable!()
        };
        *accepted_fact = false_fact;
        assert_ne!(
            selected_instruction_plan_identity(&corrupted),
            original_identity
        );
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .provenance
            .obligations[0] = ObligationId::new(9_502).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[2].constraint =
            staged.register_environment().selected_keys().copy_i64;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .provenance
            .operations[0] = OperationId::new(9_503).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .provenance
            .fuel[0]
            .units += 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
                | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
        ));
    }
}

#[test]
fn exact_subtract_retains_proof_target_effects_and_reaches_homes() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_exact_subtract_conditional(target);
        assert_eq!(
            staged.legalized().plan().functions[0].recipe,
            LegalizationRecipe::ReturnU64ExactSubtractImmediateConditionalV1
        );
        let plan = staged.selected().plan();
        assert_eq!(plan.functions[0].virtual_registers.len(), 7);
        assert_eq!(staged.selected().receipt().instruction_count(), 10);
        let accepted = &staged
            .optimized_target()
            .optimized()
            .unit()
            .accepted_obligation_facts;
        for (block, expected_obligation) in plan.functions[0].blocks[1..].iter().zip([
            ObligationId::new(5_031).unwrap(),
            ObligationId::new(5_032).unwrap(),
        ]) {
            let subtract = &block.instructions[2];
            let SelectedInstructionKind::ExactSubtractI64 {
                obligation,
                accepted_fact,
            } = subtract.kind
            else {
                panic!("leaf arithmetic must retain exact-subtract semantics")
            };
            assert_eq!(obligation, expected_obligation);
            let fact = accepted
                .iter()
                .find(|fact| fact.identity == accepted_fact)
                .expect("selected fact must remain verifier-owned");
            assert_eq!(fact.operation, subtract.provenance.operations[0]);
            assert_eq!(fact.obligation, obligation);
            assert_eq!(
                subtract.constraint,
                staged.register_environment().selected_keys().subtract_i64
            );
            assert_eq!(
                subtract
                    .operands
                    .iter()
                    .map(|operand| operand.access)
                    .collect::<Vec<_>>(),
                vec![
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Def,
                ]
            );
            assert!(subtract.implicit_uses.is_empty());
            assert!(subtract.implicit_defs.is_empty());
            if target.architecture == omega_target::Architecture::X86_64 {
                assert!(!subtract.clobbers.is_empty());
            } else {
                assert!(subtract.clobbers.is_empty());
            }
            assert_eq!(subtract.provenance.obligations, vec![obligation]);
            assert_eq!(subtract.provenance.fuel.len(), 1);
        }

        let identity = staged.selected().receipt().identity();
        let mut corrupted = plan.clone();
        let SelectedInstructionKind::ExactSubtractI64 { obligation, .. } =
            &mut corrupted.functions[0].blocks[1].instructions[2].kind
        else {
            unreachable!()
        };
        *obligation = ObligationId::new(9_504).unwrap();
        assert_ne!(selected_instruction_plan_identity(&corrupted), identity);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        let SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } = corrupted.functions[0].blocks[1].instructions[2].kind
        else {
            unreachable!()
        };
        corrupted.functions[0].blocks[1].instructions[2].kind =
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            };
        assert_ne!(selected_instruction_plan_identity(&corrupted), identity);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch { .. })
        ));

        let homes = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(
                    stage_optimized_liveness(staged).expect("subtract liveness"),
                )
                .expect("subtract ranges"),
            )
            .expect("subtract legality"),
        )
        .expect("subtract homes");
        assert_eq!(homes.custody().assignment_count(), 7);
        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert_eq!(post.custody().instruction_count(), 10);
        let decoded_post = omega_machine_optimizer::PostAllocationMachinePlan::decode(
            &post.machine().plan().encode(),
        )
        .unwrap();
        assert_eq!(&decoded_post, post.machine().plan());
        assert_eq!(
            validate_raw_post_allocation(&homes, &post, decoded_post.clone())
                .unwrap()
                .receipt(),
            post.machine().receipt()
        );
        assert_eq!(
            post.custody().source(),
            &StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(
                homes.custody()
            )
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_custody(&homes, &post).unwrap(),
            post.custody()
        );
        let selected_stage = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let encodings = stage_optimized_layout_independent_selected_form_encoding(
            selected_stage.selected(),
            &post,
            selected_stage.register_environment().physical(),
        )
        .unwrap();
        assert_eq!(encodings.selected(), post.machine().receipt().selected());
        assert_eq!(encodings.machine(), post.machine().receipt().identity());
        assert_eq!(encodings.rows().len(), 10);
        assert_eq!(
            encodings
                .rows()
                .iter()
                .filter(|row| matches!(
                    row.state,
                    SelectedFormEncodingState::DeferredControl { .. }
                ))
                .count(),
            1
        );
        assert!(encodings.rows().iter().all(|row| match &row.state {
            SelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
            SelectedFormEncodingState::DeferredControl { .. } => true,
        }));
        let returns = encodings
            .rows()
            .iter()
            .filter(|row| {
                row.alternative.family
                    == omega_selected_instructions::MachineAlternativeFamily::ReturnI64
            })
            .collect::<Vec<_>>();
        assert_eq!(returns.len(), 2);
        for returned in returns {
            let SelectedFormEncodingState::Encoded { bytes, footprint } = &returned.state else {
                panic!("returns have layout-independent target encodings")
            };
            assert_eq!(
                bytes.as_slice(),
                if target.architecture == omega_target::Architecture::X86_64 {
                    &[0xc3][..]
                } else {
                    &[0xc0, 0x03, 0x5f, 0xd6][..]
                }
            );
            assert!(footprint.register_reads.is_empty());
            assert!(footprint.register_writes.is_empty());
            assert!(footprint.encoded.external_operand_reads.is_empty());
            assert!(footprint.encoded.external_operand_writes.is_empty());
        }
        validate_optimized_layout_independent_selected_form_encoding(
            selected_stage.selected(),
            &post,
            selected_stage.register_environment().physical(),
            &encodings,
        )
        .unwrap();
        let layout = stage_optimized_resolved_selected_form_layout(
            selected_stage.selected(),
            &post,
            selected_stage.register_environment().physical(),
            &encodings,
        )
        .unwrap();
        assert_eq!(
            layout.policy(),
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
        );
        assert_eq!(layout.pre_layout(), encodings.identity());
        assert_eq!(layout.functions().len(), 1);
        let selected_function = &selected_stage.selected().plan().functions[0];
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &selected_function
            .blocks
            .iter()
            .find(|block| block.id == selected_function.entry_block)
            .unwrap()
            .terminator
        else {
            panic!("fixture entry is conditional")
        };
        let function_layout = &layout.functions()[0];
        assert_eq!(
            function_layout
                .blocks
                .iter()
                .map(|block| block.block)
                .collect::<Vec<_>>(),
            [
                selected_function.entry_block,
                when_zero.block,
                when_nonzero.block
            ]
        );
        assert!(
            function_layout
                .blocks
                .windows(2)
                .all(|pair| pair[0].offset + pair[0].byte_count == pair[1].offset)
        );
        let branch = function_layout
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|row| row.branch.as_deref().map(|branch| (row, branch)))
            .expect("one resolved branch");
        assert_eq!(branch.1.when_zero_block, when_zero.block);
        assert_eq!(branch.1.when_nonzero_block, when_nonzero.block);
        assert_eq!(
            branch.0.offset + u64::try_from(branch.0.bytes.len()).unwrap(),
            branch.1.when_zero_offset
        );
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(&branch.0.bytes[..2], [0x0f, 0x85]);
                assert_eq!(
                    branch.1.byte_displacement,
                    i64::try_from(branch.1.when_nonzero_offset).unwrap()
                        - i64::try_from(branch.0.offset + 6).unwrap()
                );
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(branch.0.bytes[0] & 0x1f, 1);
                assert_eq!(
                    branch.1.byte_displacement,
                    i64::try_from(branch.1.when_nonzero_offset).unwrap()
                        - i64::try_from(branch.0.offset).unwrap()
                );
            }
        }
        validate_optimized_resolved_selected_form_layout(
            selected_stage.selected(),
            &post,
            selected_stage.register_environment().physical(),
            &encodings,
            &layout,
        )
        .unwrap();
        let mut corrupted_layout = layout.clone();
        corrupted_layout.functions_mut()[0].blocks[0]
            .instructions
            .last_mut()
            .unwrap()
            .bytes[0] ^= 1;
        assert_eq!(
            validate_optimized_resolved_selected_form_layout(
                selected_stage.selected(),
                &post,
                selected_stage.register_environment().physical(),
                &encodings,
                &corrupted_layout,
            ),
            Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
        );
        let subtracts = post
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
        assert_eq!(subtracts.len(), 2);
        assert!(subtracts.iter().all(|instruction| {
            instruction.operands.len() == 3
                && instruction
                    .unit_uses
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                && instruction
                    .unit_defs
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                && instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.write_semantics.is_some())
                    .all(|operand| !operand.write_units.is_empty())
        }));
        let mut corrupted = decoded_post;
        let subtract = corrupted.functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|instruction| {
                instruction.alternative.key.family
                    == omega_selected_instructions::MachineAlternativeFamily::ExactSubtractI64
            })
            .unwrap();
        subtract.alternative.key.variant = u32::MAX;
        corrupted.identity = omega_machine_optimizer::post_allocation_machine_identity(&corrupted);
        assert!(matches!(
            validate_raw_post_allocation(&homes, &post, corrupted),
            Err(omega_machine_optimizer::PostAllocationMachineError::InstructionMismatch { .. })
        ));

        let mut corrupted = post.machine().plan().clone();
        corrupted.functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .flat_map(|instruction| &mut instruction.operands)
            .find(|operand| operand.write_semantics.is_some())
            .unwrap()
            .write_units
            .clear();
        corrupted.identity = omega_machine_optimizer::post_allocation_machine_identity(&corrupted);
        assert!(matches!(
            validate_raw_post_allocation(&homes, &post, corrupted),
            Err(omega_machine_optimizer::PostAllocationMachineError::InstructionMismatch { .. })
        ));

        let mut corrupted = post.machine().plan().clone();
        corrupted.effects =
            omega_machine_optimizer::PreAllocationMachineEffectIdentity::from_bytes([0x5a; 32]);
        corrupted.identity = omega_machine_optimizer::post_allocation_machine_identity(&corrupted);
        assert_eq!(
            validate_raw_post_allocation(&homes, &post, corrupted),
            Err(omega_machine_optimizer::PostAllocationMachineError::EffectRootMismatch)
        );
    }
}

#[test]
fn machine_effect_sidecar_reconstructs_subtraction_and_control_barriers() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let selected = staged_exact_subtract_conditional(target);
        let staged = stage_optimized_machine_effects(&selected).unwrap();
        assert_eq!(staged.custody().instruction_count(), 10);
        assert_eq!(
            staged.custody().source(),
            &StagedOptimizedMachineEffectSourceCustodyReceipt::Selected(selected.custody())
        );
        assert_eq!(
            &validate_optimized_machine_effect_custody(&selected, staged.effects()).unwrap(),
            staged.custody()
        );
        let instructions = staged
            .effects()
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| {
                    instruction.barrier == omega_selected_instructions::MachineBarrier::ControlFlow
                })
                .count(),
            3
        );
        let subtracts = instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::ExactSubtractI64 { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(subtracts.len(), 2);
        for subtract in subtracts {
            assert_eq!(
                subtract.alternatives.len(),
                if target.architecture == omega_target::Architecture::X86_64 {
                    4
                } else {
                    1
                }
            );
            assert_eq!(
                subtract.unit_clobbers.is_empty(),
                target.architecture != omega_target::Architecture::X86_64
            );
            assert_eq!(subtract.provenance.obligations.len(), 1);
            assert_eq!(subtract.provenance.fuel.len(), 1);
        }

        let mut corrupted = staged.effects().plan().clone();
        corrupted.functions[0].blocks[1].instructions[2]
            .alternatives
            .clear();
        assert!(matches!(
            omega_machine_optimizer::validate_pre_allocation_machine_effects(
                selected.selected(),
                selected.register_environment().identity(),
                selected.register_environment().physical(),
                selected.register_environment().constraints(),
                selected.register_environment().reservations(),
                selected.register_environment().allocation_constraint_keys(),
                &match target.architecture {
                    omega_target::Architecture::X86_64 => {
                        omega_isa_x86_64::validate_x86_64_machine_effect_catalog(
                            target,
                            selected.register_environment().constraints(),
                            omega_isa_x86_64::x86_64_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                    }
                    omega_target::Architecture::Aarch64 => {
                        omega_isa_aarch64::validate_aarch64_machine_effect_catalog(
                            target,
                            selected.register_environment().constraints(),
                            omega_isa_aarch64::aarch64_machine_effect_catalog(
                                target,
                                selected.register_environment().constraints(),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                    }
                },
                corrupted,
            ),
            Err(omega_machine_optimizer::MachineEffectError::InstructionMismatch { .. })
        ));
    }
}
