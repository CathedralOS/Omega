//! Project call provenance while physical planning still owns the instruction
//! columns. Encoding and object publication carry these immutable rows forward.

use crate::instruction_roster::SelectedInstructionRoster;
use machine_code::call_origins::{
    PhysicalCallInstructionOrigin, PhysicalCallOrigins, PhysicalFunctionCallOrigins,
    PhysicalInternalCallOrigin,
};
use selected_instructions::SelectedInstructionId;

pub fn capture(
    program: &post_allocation_machine::PostAllocationMachineProgram,
) -> PhysicalCallOrigins {
    PhysicalCallOrigins {
        psi: program.psi,
        target: program.target,
        physical_identity: program.identity,
        functions: program
            .functions
            .iter()
            .map(|function| {
                // Only functions with call records need the roster.
                let roster = if function.calls.is_empty() {
                    SelectedInstructionRoster::default()
                } else {
                    SelectedInstructionRoster::new(function.blocks.iter())
                };
                PhysicalFunctionCallOrigins {
                    machine: function.machine,
                    structural_types: function
                        .structural
                        .as_ref()
                        .map(|contract| contract.structural_types.clone()),
                    calls: function
                        .calls
                        .iter()
                        .map(|record| PhysicalInternalCallOrigin {
                            owner: record.owner,
                            instruction: record.instruction,
                            call: record.call.clone(),
                            occurrence: instruction_origin(&roster, record.instruction),
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

fn instruction_origin(
    roster: &SelectedInstructionRoster<'_>,
    instruction: SelectedInstructionId,
) -> Option<PhysicalCallInstructionOrigin> {
    let (block, row) = roster.unique(instruction)?;
    Some(PhysicalCallInstructionOrigin {
        target: row.kind.call_target(),
        block: block.id,
        callee: row.kind.machine_callee(),
        operations: row.provenance.operations.clone(),
        edges: row.provenance.edges.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedCallTarget,
        SelectedInstruction, SelectedInstructionKind, SelectedTerminator, terminator_instruction,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId};

    fn completion() -> SelectedBlock {
        let edge = EdgeId::new(7).unwrap();
        let mut provenance = selected_instructions::SelectedInstructionProvenance::default();
        provenance.edges.push(edge);
        SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::InvokeNoNormal {
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(3),
                    kind: SelectedInstructionKind::InvokeNoNormal {
                        target: SelectedCallTarget::Machine(MachineId::new(9).unwrap()),
                    },
                    constraint: register_model::RegisterConstraintKey::instruction(0),
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance,
                },
                psi_edge: edge,
            },
        }
    }

    #[test]
    fn completion_call_keeps_the_callee_and_edge_without_retaining_the_block() {
        let blocks = [completion()];
        let roster = SelectedInstructionRoster::new(&blocks);
        let origin = instruction_origin(&roster, SelectedInstructionId(3)).unwrap();
        drop(roster);
        drop(blocks);
        assert_eq!(origin.block, SelectedBlockId(0));
        assert_eq!(origin.callee, MachineId::new(9));
        assert_eq!(origin.edges, [EdgeId::new(7).unwrap()]);
        assert!(origin.operations.is_empty());
    }

    #[test]
    fn absent_or_ambiguous_instruction_does_not_acquire_call_custody() {
        let mut block = completion();
        let roster = SelectedInstructionRoster::new([&block]);
        assert!(instruction_origin(&roster, SelectedInstructionId(4)).is_none());
        block
            .instructions
            .push(terminator_instruction(&block.terminator).clone());
        let roster = SelectedInstructionRoster::new([&block]);
        assert!(instruction_origin(&roster, SelectedInstructionId(3)).is_none());
    }
}
