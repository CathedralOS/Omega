//! Snapshot edge inputs before writing any destination-associated transfer value.
use super::*;

pub(in crate::selection) fn prepare(
    function_index: usize,
    function: &mut SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let source_count = function.blocks.len();
    let mut next_instruction = instruction_count(function);
    let mut bridges = Vec::new();
    for source in &mut function.blocks {
        for successor in successors_mut(&mut source.terminator) {
            let active = successor
                .bindings
                .iter()
                .filter_map(|binding| match binding.transport {
                    SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => Some((binding.semantic, argument, parameter)),
                    SelectedValueTransport::Unused => None,
                })
                .collect::<Vec<_>>();
            if active.is_empty() {
                continue;
            }
            let bridge_id = SelectedBlockId(
                u32::try_from(source_count + bridges.len()).map_err(|_| invalid(function_index))?,
            );
            let mut continuation = successor.clone();
            continuation.role = SelectedSuccessorRole::EdgeTransferContinuation;
            continuation.fuel.clear();
            let mut instructions = Vec::new();
            let mut snapshots = Vec::new();
            let mut transfers = Vec::new();
            for phase in 0..2 {
                for (position, (semantic, argument, _)) in active.iter().enumerate() {
                    let input = if phase == 0 {
                        *argument
                    } else {
                        snapshots[position]
                    };
                    let source_register = function
                        .virtual_registers
                        .get(argument.0 as usize)
                        .cloned()
                        .ok_or_else(|| invalid(function_index))?;
                    let output = VirtualRegisterId(
                        u32::try_from(function.virtual_registers.len())
                            .map_err(|_| invalid(function_index))?,
                    );
                    let instruction_id = SelectedInstructionId(
                        u32::try_from(next_instruction).map_err(|_| invalid(function_index))?,
                    );
                    next_instruction += 1;
                    function.virtual_registers.push(VirtualRegister {
                        id: output,
                        scalar_type: source_register.scalar_type,
                        class: source_register.class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: instruction_id,
                            source_value: semantic.argument,
                        },
                        definition_site: source_register.definition_site,
                        entry_fixed_view: None,
                    });
                    instructions.push(super::super::constraints::instruction(
                        instruction_id,
                        SelectedInstructionKind::CopyI64,
                        constraints.keys.copy_i64,
                        &[input, output],
                        SelectedInstructionProvenance {
                            values: vec![semantic.argument],
                            ..Default::default()
                        },
                        catalog,
                    )?);
                    if phase == 0 {
                        snapshots.push(output);
                    } else {
                        transfers.push(output);
                    }
                }
            }
            let mut position = 0;
            for binding in &mut continuation.bindings {
                if let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport {
                    *argument = transfers[position];
                    position += 1;
                }
            }
            let jump_id = SelectedInstructionId(
                u32::try_from(next_instruction).map_err(|_| invalid(function_index))?,
            );
            next_instruction += 1;
            let jump = super::super::constraints::instruction(
                jump_id,
                SelectedInstructionKind::Jump,
                constraints.keys.jump,
                &[],
                SelectedInstructionProvenance::default(),
                catalog,
            )?;
            bridges.push(SelectedBlock {
                id: bridge_id,
                origin: SelectedBlockOrigin::EdgeTransfer {
                    edge: successor.psi_edge,
                    target: successor.source_target,
                },
                instructions,
                terminator: SelectedTerminator::Jump {
                    instruction: jump,
                    successor: continuation,
                },
            });
            successor.block = bridge_id;
            for binding in &mut successor.bindings {
                binding.transport = SelectedValueTransport::Unused;
            }
        }
    }
    function.blocks.extend(bridges);
    Ok(())
}
