//! Direct exact-subtract proof custody, machine effects, register homes, and corruption rejection.

use crate::tests::*;

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
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => false,
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
        assert_eq!(branch.1.when_fallthrough_block, when_zero.block);
        assert_eq!(branch.1.when_taken_block, when_nonzero.block);
        assert_eq!(
            branch.0.offset + u64::try_from(branch.0.bytes.len()).unwrap(),
            branch.1.when_fallthrough_offset
        );
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(&branch.0.bytes[..2], [0x0f, 0x85]);
                assert_eq!(
                    branch.1.byte_displacement,
                    i64::try_from(branch.1.when_taken_offset).unwrap()
                        - i64::try_from(branch.0.offset + 6).unwrap()
                );
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(branch.0.bytes[0] & 0x1f, 1);
                assert_eq!(
                    branch.1.byte_displacement,
                    i64::try_from(branch.1.when_taken_offset).unwrap()
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
        let mut corrupted_layout = layout.clone();
        corrupted_layout.functions_mut()[0].blocks[0]
            .instructions
            .last_mut()
            .unwrap()
            .branch
            .as_mut()
            .unwrap()
            .byte_displacement += 1;
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
        let mut corrupted_layout = layout.clone();
        corrupted_layout.functions_mut()[0].blocks[1].offset += 1;
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
        let mut corrupted_layout = layout.clone();
        corrupted_layout.functions_mut()[0].blocks.swap(1, 2);
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
