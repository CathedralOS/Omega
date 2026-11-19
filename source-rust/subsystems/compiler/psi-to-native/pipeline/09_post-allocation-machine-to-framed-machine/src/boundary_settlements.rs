//! Close boundary source correspondence before any bytes are encoded.
//! Publication later resolves these anchors and independently decodes the bytes.

use machine_code::boundary_settlements::{
    PhysicalBoundarySettlement, PhysicalBoundarySettlementFunction, PhysicalBoundarySettlements,
};
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlementPayload, SelectedFunction,
    SelectedInstructionKind, SelectedTerminator,
};
use semantic_vocabulary::{IntegerSign, ScalarType};

pub fn capture(
    program: &post_allocation_machine::PostAllocationMachineProgram,
) -> PhysicalBoundarySettlements {
    PhysicalBoundarySettlements {
        functions: program
            .functions
            .iter()
            .map(|function| PhysicalBoundarySettlementFunction {
                machine: function.machine,
                rows: capture_function(function),
            })
            .collect(),
    }
}

pub fn capture_function(
    function: &SelectedFunction,
) -> Result<Vec<PhysicalBoundarySettlement>, &'static str> {
    function
        .boundary_settlements
        .iter()
        .map(|located| {
            let mut blocks = function
                .blocks
                .iter()
                .filter(|block| block.id == located.block);
            let block = blocks.next().ok_or("settlement has no block")?;
            if blocks.next().is_some() {
                return Err("settlement has duplicate blocks");
            }
            let index = usize::try_from(located.instruction_index)
                .map_err(|_| "settlement index overflow")?;
            let instruction = block
                .instructions
                .get(index)
                .or_else(|| {
                    (index == block.instructions.len())
                        .then(|| selected_instructions::terminator_instruction(&block.terminator))
                })
                .ok_or("settlement position is outside block")?;
            let mut declaration = None;
            match &located.settlement {
                SelectedBoundarySettlementPayload::HostedReadByte {
                    operation, result, ..
                } => {
                    let slot = LocalStorageSlotId::Structural {
                        operation: *operation,
                        place: result.place,
                    };
                    if index == block.instructions.len()
                        || instruction.kind != (SelectedInstructionKind::HostedReadByte { slot })
                        || !instruction.operands.is_empty()
                        || instruction.provenance.operations.last() != Some(operation)
                        || !instruction.provenance.values.is_empty()
                    {
                        return Err("selected byte input result custody");
                    }
                    let contract = function
                        .structural
                        .as_ref()
                        .ok_or("selected byte input declaration custody")?;
                    let mut matching = contract
                        .structural_types
                        .iter()
                        .filter(|row| row.id == result.structural_type);
                    let result = matching
                        .next()
                        .ok_or("selected byte input declaration custody")?;
                    if matching.next().is_some() || !hosted_read_byte_declaration_is_valid(result) {
                        return Err("selected byte input declaration custody");
                    }
                    declaration = Some(result.clone());
                }
                SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation, source, ..
                } => {
                    let slot = LocalStorageSlotId::Boundary {
                        operation: *operation,
                    };
                    if index == block.instructions.len()
                        || instruction.kind
                            != (SelectedInstructionKind::HostedWriteByteI32 { slot })
                        || instruction.operands.len() != 1
                        || instruction.provenance.operations.last() != Some(operation)
                        || instruction.provenance.values != [*source]
                    {
                        return Err("byte output source or scratch substitution");
                    }
                }
                SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                    edge, source, ..
                } => {
                    let SelectedTerminator::HostedExitProcess {
                        completion_edge, ..
                    } = &block.terminator
                    else {
                        return Err("process exit terminal");
                    };
                    if index != block.instructions.len()
                        || edge != completion_edge
                        || instruction.kind != SelectedInstructionKind::HostedExitProcessI32
                        || instruction.operands.len() != 1
                        || instruction.provenance.edges.last() != Some(edge)
                        || instruction.provenance.values != [*source]
                    {
                        return Err("process exit source substitution");
                    }
                }
                SelectedBoundarySettlementPayload::ClaimCompletion(_) => {}
            }
            Ok(PhysicalBoundarySettlement {
                block: located.block,
                anchor: instruction.id,
                settlement: located.settlement.clone(),
                read_result_declaration: declaration,
            })
        })
        .collect()
}

fn hosted_read_byte_declaration_is_valid(
    declaration: &terminal_psi::StructuralTypeDeclaration,
) -> bool {
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return false;
    };
    let [empty, byte] = cases.as_slice() else {
        return false;
    };
    let [field] = byte.fields.as_slice() else {
        return false;
    };
    empty.fields.is_empty()
        && declaration.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && empty.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && byte.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && field.identity != terminal_psi::TerminalIdentityIndex::ABSENT
        && empty.id < byte.id
        && empty.identity != byte.identity
        && !field.relevance.is_erased()
        && match field.field_type {
            terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                !integer.is_address()
                    && integer.sign() == IntegerSign::Signed
                    && integer.bits() == Some(32)
            }
            terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                let integer = bounds.integer_type();
                !integer.is_address()
                    && integer.sign() == IntegerSign::Signed
                    && integer.bits() == Some(32)
                    && bounds.contains(semantic_vocabulary::IntegerValue::Signed(0))
                    && bounds.contains(semantic_vocabulary::IntegerValue::Signed(255))
            }
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
        SelectedFunctionContracts, SelectedInstruction, SelectedInstructionId,
        SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, EdgeId, MachineId, OperationId, ValueId,
    };

    fn fixture() -> SelectedFunction {
        let operation = OperationId::new(1).unwrap();
        let edge = EdgeId::new(2).unwrap();
        let value = ValueId::new(3).unwrap();
        let boundary = BoundaryMachineId::new(4).unwrap();
        let block = SelectedBlockId(0);
        let instruction = |id, kind, provenance| SelectedInstruction {
            id: SelectedInstructionId(id),
            kind,
            constraint: register_model::RegisterConstraintKey::instruction(0),
            operands: vec![SelectedOperand {
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: register_model::RegisterOperandAccess::Use,
                class: register_model::RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }],
            implicit_uses: vec![],
            implicit_defs: vec![],
            clobbers: vec![],
            provenance,
        };
        SelectedFunction {
            contracts: std::sync::Arc::new(SelectedFunctionContracts {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: target_vocabulary::TerminalPsiProvenance {
                    operations: vec![operation],
                    edges: vec![edge],
                },
                structural: None,
                structural_block_parameters: vec![],
                local_storage_slots: vec![],
                outgoing_arguments: vec![],
                calls: vec![],
                normalized_foreign_calls: vec![],
                memory_accesses: vec![],
                virtual_registers: vec![],
                boundary_settlements: vec![
                    SelectedBoundarySettlement {
                        block,
                        instruction_index: 0,
                        settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                            operation,
                            boundary,
                            source: value,
                            execution: target_vocabulary::BoundaryExecutionBinding::CompilerBuiltin(
                                target_vocabulary::CompilerBuiltinExecution::HostedWriteByteI32,
                            ),
                        },
                    },
                    SelectedBoundarySettlement {
                        block,
                        instruction_index: 1,
                        settlement: SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                            edge,
                            boundary,
                            source: value,
                            execution: target_vocabulary::BoundaryExecutionBinding::CompilerBuiltin(
                                target_vocabulary::CompilerBuiltinExecution::HostedExitProcessI32,
                            ),
                        },
                    },
                ],
            }),
            entry_block: block,
            blocks: vec![SelectedBlock {
                id: block,
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![instruction(
                    10,
                    SelectedInstructionKind::HostedWriteByteI32 {
                        slot: LocalStorageSlotId::Boundary { operation },
                    },
                    SelectedInstructionProvenance {
                        operations: vec![operation],
                        values: vec![value],
                        ..Default::default()
                    },
                )],
                terminator: SelectedTerminator::HostedExitProcess {
                    completion_edge: edge,
                    instruction: instruction(
                        11,
                        SelectedInstructionKind::HostedExitProcessI32,
                        SelectedInstructionProvenance {
                            edges: vec![edge],
                            values: vec![value],
                            ..Default::default()
                        },
                    ),
                },
            }]
            .into(),
        }
    }

    #[test]
    fn boundary_anchors_preserve_ordinary_and_nonreturning_occurrences() {
        let rows = capture_function(&fixture()).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.anchor).collect::<Vec<_>>(),
            vec![SelectedInstructionId(10), SelectedInstructionId(11)]
        );
        let mut changed = fixture();
        changed.boundary_settlements[1].instruction_index = 0;
        assert_eq!(
            capture_function(&changed),
            Err("process exit source substitution")
        );
        let mut changed = fixture();
        if let SelectedTerminator::HostedExitProcess {
            completion_edge, ..
        } = &mut changed.blocks[0].terminator
        {
            *completion_edge = EdgeId::new(9).unwrap();
        }
        assert_eq!(
            capture_function(&changed),
            Err("process exit source substitution")
        );
    }

    #[test]
    fn boundary_sources_and_scratch_slots_cannot_be_substituted() {
        let mut changed = fixture();
        changed.blocks[0].instructions[0].provenance.values.clear();
        assert_eq!(
            capture_function(&changed),
            Err("byte output source or scratch substitution")
        );
        let mut changed = fixture();
        changed.blocks[0].instructions[0].kind = SelectedInstructionKind::HostedWriteByteI32 {
            slot: LocalStorageSlotId::Boundary {
                operation: OperationId::new(7).unwrap(),
            },
        };
        assert_eq!(
            capture_function(&changed),
            Err("byte output source or scratch substitution")
        );
        let mut changed = fixture();
        changed.blocks[0].instructions[0]
            .provenance
            .operations
            .clear();
        assert_eq!(
            capture_function(&changed),
            Err("byte output source or scratch substitution")
        );
    }
}
