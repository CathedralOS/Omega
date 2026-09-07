//! Independent scalar-function and block liveness replay.

use super::constraints::reject_v1_unsupported;
use super::shared::*;

pub(super) fn replay_function(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<FunctionLiveness, LivenessError> {
    reject_v1_unsupported(function_index, function)?;
    super::super::edge_values::validate_transports(function_index, function)?;
    let block_ids = function
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    let mut v_in = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut v_out = v_in.clone();
    let mut u_in = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut u_out = u_in.clone();
    loop {
        let old = (v_in.clone(), v_out.clone(), u_in.clone(), u_out.clone());
        for block in function.blocks.iter().rev() {
            let targets = match &block.terminator {
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => vec![when_nonzero, when_zero],
                SelectedTerminator::ConditionalBranchU64LessThan {
                    when_less,
                    when_not_less,
                    ..
                }
                | SelectedTerminator::ConditionalBranchI64LessThan {
                    when_less,
                    when_not_less,
                    ..
                } => vec![when_less, when_not_less],
                SelectedTerminator::Jump { successor, .. } => vec![successor],
                SelectedTerminator::Return { .. } => Vec::new(),
            };
            let mut vo = BTreeSet::new();
            for edge in &targets {
                for destination in &v_in[&edge.block] {
                    vo.insert(super::super::edge_values::incoming_argument(
                        function_index,
                        function,
                        edge,
                        *destination,
                    )?);
                }
            }
            let uo = targets
                .iter()
                .filter_map(|target| u_in.get(&target.block))
                .flat_map(|set| set.iter().copied())
                .collect::<BTreeSet<_>>();
            let mut vi = vo.clone();
            let mut ui = uo.clone();
            for instruction in ordered_instructions(block).into_iter().rev() {
                for operand in &instruction.operands {
                    if operand.access == RegisterOperandAccess::Def {
                        vi.remove(&operand.virtual_register);
                    }
                }
                for operand in &instruction.operands {
                    if operand.access == RegisterOperandAccess::Use {
                        vi.insert(operand.virtual_register);
                    }
                }
                for unit in instruction
                    .implicit_defs
                    .iter()
                    .chain(&instruction.clobbers)
                {
                    ui.remove(unit);
                }
                ui.extend(instruction.implicit_uses.iter().copied());
            }
            v_out.insert(block.id, vo);
            u_out.insert(block.id, uo);
            v_in.insert(block.id, vi);
            u_in.insert(block.id, ui);
        }
        if old == (v_in.clone(), v_out.clone(), u_in.clone(), u_out.clone()) {
            break;
        }
    }

    let mut ordinal = 0_u32;
    let mut position = BTreeMap::new();
    for block in &function.blocks {
        for instruction in ordered_instructions(block) {
            position.insert(instruction.id, LivenessPosition(ordinal));
            ordinal = ordinal
                .checked_add(1)
                .ok_or(LivenessError::NonDensePositions {
                    function: function_index,
                })?;
        }
    }
    let entry_definitions = function
        .virtual_registers
        .iter()
        .filter(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::EntryParameter { .. }
            )
        })
        .map(|register| EntryDefinition {
            virtual_register: register.id,
            class: register.class,
            fixed_view: register.entry_fixed_view,
        })
        .collect();
    let operand_positions = function
        .blocks
        .iter()
        .flat_map(ordered_instructions)
        .flat_map(|instruction| {
            let instruction_position = position[&instruction.id];
            instruction
                .operands
                .iter()
                .map(move |operand| OperandPosition {
                    position: instruction_position,
                    instruction: instruction.id,
                    operand: operand.operand,
                    virtual_register: operand.virtual_register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
        })
        .collect();
    let blocks = function
        .blocks
        .iter()
        .map(|block| {
            replay_block(
                function_index,
                function,
                block,
                &position,
                &v_in,
                &v_out,
                &u_in,
                &u_out,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FunctionLiveness {
        machine: function.machine,
        entry_definitions,
        operand_positions,
        blocks,
    })
}

fn replay_block(
    function_index: usize,
    function: &SelectedFunction,
    block: &SelectedBlock,
    position: &BTreeMap<selected_instructions::SelectedInstructionId, LivenessPosition>,
    v_in: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<VirtualRegisterId>>,
    v_out: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<VirtualRegisterId>>,
    u_in: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<RegisterUnitId>>,
    u_out: &BTreeMap<selected_instructions::SelectedBlockId, BTreeSet<RegisterUnitId>>,
) -> Result<BlockLiveness, LivenessError> {
    let mut vl = v_out[&block.id].clone();
    let mut ul = u_out[&block.id].clone();
    let mut instructions = Vec::new();
    for instruction in ordered_instructions(block).into_iter().rev() {
        let vlo = collect(&vl);
        let ulo = collect(&ul);
        let uses = instruction
            .operands
            .iter()
            .filter(|operand| operand.access == RegisterOperandAccess::Use)
            .map(|operand| operand.virtual_register)
            .collect::<BTreeSet<_>>();
        let defs = instruction
            .operands
            .iter()
            .filter(|operand| operand.access == RegisterOperandAccess::Def)
            .map(|operand| operand.virtual_register)
            .collect::<BTreeSet<_>>();
        for value in &defs {
            vl.remove(value);
        }
        vl.extend(uses.iter().copied());
        for unit in instruction
            .implicit_defs
            .iter()
            .chain(&instruction.clobbers)
        {
            ul.remove(unit);
        }
        ul.extend(instruction.implicit_uses.iter().copied());
        instructions.push(InstructionLiveness {
            position: position[&instruction.id],
            instruction: instruction.id,
            virtual_uses: collect(&uses),
            virtual_defs: collect(&defs),
            virtual_live_in: collect(&vl),
            virtual_live_out: vlo,
            unit_uses: instruction.implicit_uses.clone(),
            unit_defs: instruction.implicit_defs.clone(),
            unit_clobbers: instruction.clobbers.clone(),
            unit_live_in: collect(&ul),
            unit_live_out: ulo,
        });
    }
    instructions.reverse();
    let successor_rows = match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => [when_nonzero, when_zero]
            .into_iter()
            .enumerate()
            .map(|(ordinal, successor)| SuccessorLiveness {
                terminator: instruction.id,
                polarity_ordinal: ordinal as u8,
                psi_edge: successor.psi_edge,
                target: successor.block,
                virtual_live: collect(&v_in[&successor.block]),
                unit_live: collect(&u_in[&successor.block]),
            })
            .collect(),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => [when_less, when_not_less]
            .into_iter()
            .enumerate()
            .map(|(ordinal, successor)| SuccessorLiveness {
                terminator: instruction.id,
                polarity_ordinal: ordinal as u8,
                psi_edge: successor.psi_edge,
                target: successor.block,
                virtual_live: collect(&v_in[&successor.block]),
                unit_live: collect(&u_in[&successor.block]),
            })
            .collect(),
        SelectedTerminator::Return { .. } => Vec::new(),
        SelectedTerminator::Jump {
            instruction,
            successor,
        } => vec![SuccessorLiveness {
            terminator: instruction.id,
            polarity_ordinal: 0,
            psi_edge: successor.psi_edge,
            target: successor.block,
            virtual_live: collect(&v_in[&successor.block]),
            unit_live: collect(&u_in[&successor.block]),
        }],
    };
    let mut successors = Vec::new();
    for mut row in successor_rows {
        let edge = match &block.terminator {
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => [when_nonzero, when_zero][usize::from(row.polarity_ordinal)],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => [when_less, when_not_less][usize::from(row.polarity_ordinal)],
            SelectedTerminator::Jump { successor, .. } => successor,
            SelectedTerminator::Return { .. } => unreachable!("return has no successor rows"),
        };
        let mut incoming = BTreeSet::new();
        for destination in row.virtual_live {
            incoming.insert(super::super::edge_values::incoming_argument(
                function_index,
                function,
                edge,
                destination,
            )?);
        }
        row.virtual_live = collect(&incoming);
        successors.push(row);
    }
    Ok(BlockLiveness {
        block: block.id,
        source_block: block.source_block,
        virtual_live_in: collect(&v_in[&block.id]),
        virtual_live_out: collect(&v_out[&block.id]),
        unit_live_in: collect(&u_in[&block.id]),
        unit_live_out: collect(&u_out[&block.id]),
        instructions,
        successors,
    })
}
