use encoding_admission::layout::{derive_order, validate_function, validate_row};

#[cfg(test)]
mod order_tests {
    fn derive_order(
        function: &selected_instructions::SelectedFunction,
        policy: crate::SelectedFunctionLayoutPolicy,
    ) -> Result<
        Vec<&selected_instructions::SelectedBlock>,
        crate::OptimizedResolvedSelectedFormLayoutError,
    > {
        crate::order_suite::projected_order(function, policy, super::derive_order)
    }
    crate::order_suite::order_suite!(derive_order);
}

#[cfg(test)]
mod tests {
    use isa_aarch64::{
        aarch64_physical_register_model, encode_aarch64_selected_conditional_branch_form,
        encode_aarch64_selected_jump_form,
    };
    use machine_code::{
        DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
        SelectedFormEncodingState, SelectedFormMachineDisposition,
    };
    use post_allocation_machine::{
        PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineInstruction,
    };
    use selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlock,
        SelectedFunction, SelectedInstruction, SelectedInstructionKind, SelectedTerminator,
    };
    use semantic_vocabulary::EdgeId;
    use target::Architecture;

    fn validate_function(
        architecture: Architecture,
        selected: &SelectedFunction,
        machine: &PostAllocationMachineFunction,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        policy: SelectedFunctionLayoutPolicy,
        rows: &mut std::slice::Iter<'_, SelectedFormEncodingRow>,
        candidate: &ResolvedSelectedFunctionLayout,
        encoded: &machine_code::SelectedFormEncodedRowStore,
        short: bool,
    ) -> Result<(), crate::OptimizedResolvedSelectedFormLayoutError> {
        use encoding_admission::layout_requirements::*;
        super::validate_function(
            architecture,
            &prepare_function_control(selected),
            &prepare_function_alternatives(machine),
            physical,
            policy,
            rows,
            candidate,
            encoded,
            short,
        )
    }
    fn validate_row(
        architecture: Architecture,
        block: &SelectedBlock,
        instruction: &SelectedInstruction,
        machine: &PostAllocationMachineInstruction,
        pre: &SelectedFormEncodingRow,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        offset: u64,
        offsets: &std::collections::BTreeMap<selected_instructions::SelectedBlockId, u64>,
        candidate: &crate::ResolvedSelectedFormRow,
        widened: bool,
        encoded: &machine_code::SelectedFormEncodedRowStore,
        short: bool,
    ) -> Result<(), crate::OptimizedResolvedSelectedFormLayoutError> {
        use encoding_admission::layout_requirements::*;
        super::validate_row(
            architecture,
            &prepare_block_control(block),
            &prepare_row_control(instruction),
            &prepare_alternative(machine),
            pre,
            physical,
            offset,
            offsets,
            candidate,
            widened,
            encoded,
            short,
        )
    }
    use crate::order_suite::{block_with, branch, function_from_blocks, instruction, successor};
    fn layout<'machine>(
        architecture: Architecture,
        selected: &SelectedFunction,
        rows: &[SelectedFormEncodingRow],
        machine: impl Iterator<Item = &'machine PostAllocationMachineInstruction>,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        policy: SelectedFunctionLayoutPolicy,
        encoded: &machine_code::SelectedFormEncodedRowStore,
    ) -> Result<ResolvedSelectedFunctionLayout, crate::OptimizedResolvedSelectedFormLayoutError>
    {
        use encoding_admission::layout::geometry::InstructionRows;
        use encoding_admission::layout_requirements::*;
        let rows = InstructionRows::new(rows.iter().map(|row| (row.instruction, row))).unwrap();
        let requirements: Vec<_> = machine.map(prepare_alternative).collect();
        let alternatives =
            InstructionRows::new(requirements.iter().map(|row| (row.instruction, row))).unwrap();
        crate::ordinary::layout(
            architecture,
            &prepare_function_control(selected),
            &rows,
            &alternatives,
            physical,
            policy,
            encoded,
        )
    }
    use crate::{
        ResolvedBranchEvidence, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
    };

    const PAD_BEYOND_IMM19: usize = (1 << 18) + 4;

    fn jump(source: u32, destination: u32) -> SelectedTerminator {
        SelectedTerminator::Jump {
            instruction: instruction(source, SelectedInstructionKind::Jump),
            successor: successor(destination),
        }
    }

    fn returned(source: u32) -> SelectedTerminator {
        SelectedTerminator::Return {
            instruction: instruction(source, SelectedInstructionKind::ReturnUnit),
            psi_return_edge: EdgeId::new(u64::from(source) + 1).unwrap(),
        }
    }

    fn block(id: u32, terminator: SelectedTerminator) -> SelectedBlock {
        block_with(id, Vec::new(), terminator)
    }

    fn physical() -> register_model::ValidatedPhysicalRegisterModel {
        register_model::validate_physical_register_model(aarch64_physical_register_model()).unwrap()
    }

    fn branch_alternative() -> MachineAlternative {
        let physical = physical();
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        };
        let encoded = encode_aarch64_selected_conditional_branch_form(
            &physical,
            key,
            MachineAlternativeFamily::ConditionalBranchNonZero,
            isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1,
            8,
        )
        .unwrap();
        MachineAlternative {
            key,
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            },
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: encoded.footprint().encoded.clone(),
        }
    }

    fn jump_alternative() -> MachineAlternative {
        let physical = physical();
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        };
        let encoded = encode_aarch64_selected_jump_form(&physical, key, 4).unwrap();
        MachineAlternative {
            key,
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: encoded.footprint().encoded.clone(),
        }
    }

    fn plain_alternative() -> MachineAlternative {
        MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: branch_alternative().encoded,
        }
    }

    fn row_alternative(
        instruction: &SelectedInstruction,
        alternatives: &Alternatives,
    ) -> MachineAlternativeKey {
        alternatives.of(instruction).key
    }

    fn footprint(alternatives: &Alternatives) -> SelectedFormDecodedFootprint {
        SelectedFormDecodedFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            implicit_defs: Vec::new(),
            implicit_clobbers: Vec::new(),
            encoded: alternatives.plain.encoded.clone(),
        }
    }

    fn encoded_row(
        instruction: &SelectedInstruction,
        footprint: &SelectedFormDecodedFootprint,
        encoded: &mut machine_code::SelectedFormEncodedRowStore,
    ) -> SelectedFormEncodingRow {
        let (code, interned) = encoded.intern(&[0; 4], footprint.clone());
        SelectedFormEncodingRow {
            instruction: instruction.id,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyI64,
                variant: 0,
            },
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            state: SelectedFormEncodingState::Encoded,
            address: None,
            code,
            footprint: interned,
        }
    }

    fn control_templates(
        key: MachineAlternativeKey,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        architecture: target::Architecture,
    ) -> machine_code::EncodedControlAlternatives {
        use machine_code::{
            BranchDisplacementField as Field, EncodedBranchAlternative, EncodedControlAlternatives,
        };
        macro_rules! form {
            ($encoded:expr, $key:expr, $field:expr, $offset:expr, $reference:expr) => {{
                let encoded = $encoded.unwrap();
                let fp = encoded.footprint();
                EncodedBranchAlternative {
                    alternative: $key,
                    bytes: encoded.bytes().to_vec(),
                    field: $field,
                    field_offset: $offset,
                    reference_offset: $reference,
                    footprint: SelectedFormDecodedFootprint {
                        register_reads: fp.register_reads.clone(),
                        register_writes: fp.register_writes.clone(),
                        implicit_defs: fp.encoded.implicit_unit_defs.clone(),
                        implicit_clobbers: fp.encoded.implicit_unit_clobbers.clone(),
                        encoded: fp.encoded.clone(),
                    },
                }
            }};
        }
        match architecture {
            target::Architecture::Aarch64 if key.family == MachineAlternativeFamily::Jump => {
                EncodedControlAlternatives {
                    baseline: form!(
                        encode_aarch64_selected_jump_form(physical, key, 0),
                        key,
                        Field::Aarch64Branch26,
                        0,
                        0
                    ),
                    widened: None,
                    shortened: None,
                }
            }
            target::Architecture::Aarch64 => EncodedControlAlternatives {
                baseline: form!(
                    encode_aarch64_selected_conditional_branch_form(
                        physical,
                        key,
                        key.family,
                        isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1,
                        0
                    ),
                    key,
                    Field::Aarch64Conditional19,
                    0,
                    0
                ),
                widened: Some(form!(
                    encode_aarch64_selected_conditional_branch_form(
                        physical,
                        key,
                        key.family,
                        isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1,
                        4
                    ),
                    key,
                    Field::Aarch64Branch26,
                    4,
                    4
                )),
                shortened: None,
            },
            target::Architecture::X86_64 => {
                let encode = |key: MachineAlternativeKey, short| match key.family {
                    MachineAlternativeFamily::ConditionalBranchNonZero if short => {
                        isa_x86_64::encode_x86_64_selected_short_nonzero_branch_form(
                            physical, key, 0,
                        )
                    }
                    MachineAlternativeFamily::ConditionalBranchNonZero => {
                        isa_x86_64::encode_x86_64_selected_nonzero_branch_form(physical, key, 0)
                    }
                    MachineAlternativeFamily::ConditionalBranchU64LessThan => {
                        isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                            physical, key, 0,
                        )
                    }
                    MachineAlternativeFamily::ConditionalBranchI64LessThan => {
                        isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                            physical, key, 0,
                        )
                    }
                    _ => panic!("fixture requires a conditional branch"),
                };
                let short = MachineAlternativeKey {
                    variant: u32::from(
                        key.family != MachineAlternativeFamily::ConditionalBranchNonZero,
                    ),
                    ..key
                };
                EncodedControlAlternatives {
                    baseline: form!(encode(key, false), key, Field::Relative32, 2, 6),
                    shortened: Some(form!(encode(short, true), short, Field::Relative8, 1, 2)),
                    widened: None,
                }
            }
        }
    }

    fn deferred_row(
        instruction: &SelectedInstruction,
        alternatives: &Alternatives,
    ) -> SelectedFormEncodingRow {
        SelectedFormEncodingRow {
            instruction: instruction.id,
            alternative: row_alternative(instruction, alternatives),
            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
            state: SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                alternatives: std::sync::Arc::new(control_templates(
                    row_alternative(instruction, alternatives),
                    &physical(),
                    target::Architecture::Aarch64,
                )),
            },
            address: None,
            code: machine_code::SelectedFormEncodedByteSpan::default(),
            footprint: machine_code::SelectedFormDecodedFootprintHandle::default(),
        }
    }

    fn terminator_row(terminator: &SelectedTerminator) -> &SelectedInstruction {
        selected_instructions::terminator_instruction(terminator)
    }

    fn machine_row(
        instruction: &SelectedInstruction,
        alternative: &MachineAlternative,
    ) -> PostAllocationMachineInstruction {
        PostAllocationMachineInstruction {
            instruction: instruction.id,
            alternative: alternative.clone(),
            operands: Vec::new(),
            address: None,
            implicit_unit_uses: Vec::new(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: Vec::new(),
            unit_uses: Vec::new(),
            unit_defs: Vec::new(),
            unit_clobbers: Vec::new(),
        }
    }

    fn pre_layout_rows(
        function: &SelectedFunction,
        footprint: &SelectedFormDecodedFootprint,
        alternatives: &Alternatives,
        encoded: &mut machine_code::SelectedFormEncodedRowStore,
    ) -> Vec<SelectedFormEncodingRow> {
        let mut rows = Vec::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                rows.push(encoded_row(instruction, footprint, encoded));
            }
            let terminator = terminator_row(&block.terminator);
            rows.push(match &block.terminator {
                SelectedTerminator::ConditionalBranch { .. }
                | selected_instructions::conditional_branch_less_pair_pattern!(..)
                | SelectedTerminator::Jump { .. } => deferred_row(terminator, alternatives),
                _ => encoded_row(terminator, footprint, encoded),
            });
        }
        rows
    }

    fn machine_function(
        function: &SelectedFunction,
        alternatives: &Alternatives,
    ) -> PostAllocationMachineFunction {
        PostAllocationMachineFunction {
            machine: function.machine,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            blocks: function
                .blocks
                .iter()
                .map(|block| PostAllocationMachineBlock {
                    block: block.id,
                    instructions: block
                        .instructions
                        .iter()
                        .chain(std::iter::once(terminator_row(&block.terminator)))
                        .map(|instruction| machine_row(instruction, alternatives.of(instruction)))
                        .collect(),
                })
                .collect(),
        }
    }

    struct Alternatives {
        branch: MachineAlternative,
        jump: MachineAlternative,
        plain: MachineAlternative,
    }

    impl Alternatives {
        fn of(&self, instruction: &SelectedInstruction) -> &MachineAlternative {
            match instruction.kind {
                SelectedInstructionKind::ConditionalBranchNonZero => &self.branch,
                SelectedInstructionKind::Jump => &self.jump,
                _ => &self.plain,
            }
        }
    }

    fn produce_and_admit(selected: &SelectedFunction) -> ResolvedSelectedFunctionLayout {
        let alternatives = Alternatives {
            branch: branch_alternative(),
            jump: jump_alternative(),
            plain: plain_alternative(),
        };
        let footprint = footprint(&alternatives);
        let mut encoded = machine_code::SelectedFormEncodedRowStore::new();
        let rows = pre_layout_rows(selected, &footprint, &alternatives, &mut encoded);
        let machine = machine_function(selected, &alternatives);
        let produced = layout(
            Architecture::Aarch64,
            selected,
            &rows,
            machine.instructions(),
            &physical(),
            SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
            &encoded,
        )
        .unwrap();
        let mut iter = rows.iter();
        validate_function(
            Architecture::Aarch64,
            selected,
            &machine,
            &physical(),
            SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
            &mut iter,
            &produced,
            &encoded,
            false,
        )
        .unwrap();
        assert!(iter.next().is_none());
        produced
    }

    #[test]
    fn current_x86_short_rows_require_selection_exact_bytes_and_successors() {
        use machine_code::{ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate};
        let physical = isa_x86_64::validated_x86_64_physical_register_model();
        for predicate in [
            ResolvedConditionalBranchPredicate::NonZeroV1,
            ResolvedConditionalBranchPredicate::U64LessThanV1,
            ResolvedConditionalBranchPredicate::I64LessThanV1,
        ] {
            let (kind, family) = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => (
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    MachineAlternativeFamily::ConditionalBranchNonZero,
                ),
                ResolvedConditionalBranchPredicate::U64LessThanV1 => (
                    SelectedInstructionKind::ConditionalBranchU64LessThan,
                    MachineAlternativeFamily::ConditionalBranchU64LessThan,
                ),
                ResolvedConditionalBranchPredicate::I64LessThanV1 => (
                    SelectedInstructionKind::ConditionalBranchI64LessThan,
                    MachineAlternativeFamily::ConditionalBranchI64LessThan,
                ),
            };
            let instruction = instruction(0, kind);
            let terminator = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    SelectedTerminator::ConditionalBranch {
                        instruction: instruction.clone(),
                        when_nonzero: successor(2),
                        when_zero: successor(1),
                    }
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    SelectedTerminator::ConditionalBranchU64LessThan {
                        instruction: instruction.clone(),
                        when_less: successor(2),
                        when_not_less: successor(1),
                    }
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    SelectedTerminator::ConditionalBranchI64LessThan {
                        instruction: instruction.clone(),
                        when_less: successor(2),
                        when_not_less: successor(1),
                    }
                }
            };
            let block = block(0, terminator);
            let near_key = MachineAlternativeKey { family, variant: 0 };
            let short_key = MachineAlternativeKey {
                family,
                variant: u32::from(predicate != ResolvedConditionalBranchPredicate::NonZeroV1),
            };
            let near = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    isa_x86_64::encode_x86_64_selected_nonzero_branch_form(physical, near_key, 1)
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                        physical, near_key, 1,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                        physical, near_key, 1,
                    )
                }
            }
            .unwrap();
            let short = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    isa_x86_64::encode_x86_64_selected_short_nonzero_branch_form(
                        physical, short_key, 1,
                    )
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                        physical, short_key, 1,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                        physical, short_key, 1,
                    )
                }
            }
            .unwrap();
            let alternative = MachineAlternative {
                key: near_key,
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(6),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: near.footprint().encoded.clone(),
            };
            let machine = machine_row(&instruction, &alternative);
            let pre = SelectedFormEncodingRow {
                instruction: instruction.id,
                alternative: near_key,
                machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                state: SelectedFormEncodingState::DeferredControl {
                    reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                    alternatives: std::sync::Arc::new(control_templates(
                        near_key,
                        physical,
                        target::Architecture::X86_64,
                    )),
                },
                address: None,
                code: Default::default(),
                footprint: Default::default(),
            };
            let row = machine_code::ResolvedSelectedFormRow {
                instruction: instruction.id,
                alternative: short_key,
                offset: 0,
                bytes: short.bytes().to_vec(),
                branch: Some(Box::new(ResolvedBranchEvidence::Conditional(
                    ResolvedConditionalBranchEvidence {
                        predicate,
                        source_block: block.id,
                        when_taken_edge: successor(2).psi_edge,
                        when_taken_block: successor(2).block,
                        when_taken_offset: 3,
                        when_fallthrough_edge: successor(1).psi_edge,
                        when_fallthrough_block: successor(1).block,
                        when_fallthrough_offset: 2,
                        byte_displacement: 1,
                        decoded_register_reads: short.footprint().register_reads.clone(),
                        decoded_effects: short.footprint().encoded.clone(),
                    },
                ))),
                internal_machine_fixup: None,
                normalized_foreign_call_fixup: None,
                image_address_fixup: None,
            };
            let offsets = [(successor(1).block, 2), (successor(2).block, 3)]
                .into_iter()
                .collect();
            let encoded = machine_code::SelectedFormEncodedRowStore::new();
            let check = |row, allowed| {
                validate_row(
                    Architecture::X86_64,
                    &block,
                    &instruction,
                    &machine,
                    &pre,
                    physical,
                    0,
                    &offsets,
                    row,
                    false,
                    &encoded,
                    allowed,
                )
            };
            check(&row, true).unwrap();
            assert!(
                check(&row, false).is_err(),
                "baseline validation cannot admit a relaxed row"
            );
            let mut changed = row.clone();
            changed.bytes[1] = 2;
            assert!(
                check(&changed, true).is_err(),
                "branch displacement is decoded independently"
            );
            let mut changed = row.clone();
            let Some(ResolvedBranchEvidence::Conditional(branch)) = changed.branch.as_deref_mut()
            else {
                unreachable!()
            };
            branch.when_taken_edge = successor(1).psi_edge;
            assert!(
                check(&changed, true).is_err(),
                "equal offsets do not authorize another edge"
            );
            let mut changed = row.clone();
            changed.alternative.variant = 9;
            assert!(
                check(&changed, true).is_err(),
                "only the exact short alternative is admitted"
            );
        }
    }

    #[test]
    fn taken_edge_beyond_imm19_widens_to_inverted_skip_plus_unconditional_branch() {
        let padding: Vec<SelectedInstruction> = (0..PAD_BEYOND_IMM19 as u32)
            .map(|index| instruction(1000 + index, SelectedInstructionKind::CopyI64))
            .collect();
        let selected = function_from_blocks(vec![
            block(0, branch(0, 2, 1)),
            block_with(1, padding, jump(1, 2)),
            block(2, returned(2)),
        ]);
        let layout = produce_and_admit(&selected);
        let row = &layout.blocks[0].instructions[0];
        assert_eq!(row.bytes.len(), 8);
        let skip = u32::from_le_bytes(row.bytes[0..4].try_into().unwrap());
        assert_eq!(skip, 0x5400_0000 | (2 << 5));
        let target = u32::from_le_bytes(row.bytes[4..8].try_into().unwrap());
        assert_eq!(target & 0xfc00_0000, 0x1400_0000);
        let branch_offset = layout.blocks[0].instructions[0].offset;
        let taken_offset = layout.blocks[2].offset;
        let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
        assert_eq!(
            decoded,
            i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap() - 4
        );
        assert!(
            i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap() > 1 << 20
        );
        let Some(ResolvedBranchEvidence::Conditional(evidence)) =
            layout.blocks[0].instructions[0].branch.as_deref()
        else {
            panic!("conditional branch must carry conditional evidence")
        };
        assert_eq!(
            evidence.byte_displacement,
            i64::try_from(taken_offset).unwrap() - i64::try_from(branch_offset).unwrap()
        );
        let jump_row = layout.blocks[1].instructions.last().unwrap();
        assert_eq!(jump_row.bytes.len(), 4);
        assert_eq!(
            u32::from_le_bytes(jump_row.bytes[..4].try_into().unwrap()) & 0xfc00_0000,
            0x1400_0000
        );
    }

    #[test]
    fn backward_taken_edge_beyond_imm19_widens() {
        let padding: Vec<SelectedInstruction> = (0..PAD_BEYOND_IMM19 as u32)
            .map(|index| instruction(2000 + index, SelectedInstructionKind::CopyI64))
            .collect();
        let selected = function_from_blocks(vec![
            block(0, jump(0, 1)),
            block_with(1, padding, branch(1, 0, 2)),
            block(2, returned(2)),
        ]);
        let layout = produce_and_admit(&selected);
        let row = layout.blocks[1].instructions.last().unwrap();
        assert_eq!(row.bytes.len(), 8);
        let skip = u32::from_le_bytes(row.bytes[0..4].try_into().unwrap());
        assert_eq!(skip, 0x5400_0000 | (2 << 5));
        let target = u32::from_le_bytes(row.bytes[4..8].try_into().unwrap());
        let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
        assert_eq!(decoded, -(i64::try_from(row.offset).unwrap()) - 4);
    }

    #[test]
    fn in_range_branch_keeps_the_canonical_four_byte_form() {
        let padding: Vec<SelectedInstruction> = (0..4u32)
            .map(|index| instruction(3000 + index, SelectedInstructionKind::CopyI64))
            .collect();
        let selected = function_from_blocks(vec![
            block(0, branch(0, 2, 1)),
            block_with(1, padding, jump(1, 2)),
            block(2, returned(2)),
        ]);
        let layout = produce_and_admit(&selected);
        let row = &layout.blocks[0].instructions[0];
        assert_eq!(row.bytes.len(), 4);
        let word = u32::from_le_bytes(row.bytes[..4].try_into().unwrap());
        assert_eq!(word & 0xff00_001f, 0x5400_0001);
    }
}
