//! Project call provenance while physical planning still owns the instruction
//! columns. Encoding and object publication carry these immutable rows forward.

use machine_code::call_origins::{
    PhysicalCallInstructionOrigin, PhysicalCallOrigins, PhysicalFunctionCallOrigins,
    PhysicalInternalCallOrigin,
};
use selected_instructions::{SelectedBlock, SelectedInstructionId, terminator_instruction};

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
            .map(|function| PhysicalFunctionCallOrigins {
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
                        occurrence: instruction_origin(function.blocks.iter(), record.instruction),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn instruction_origin<'a>(
    blocks: impl Iterator<Item = &'a SelectedBlock>,
    instruction: SelectedInstructionId,
) -> Option<PhysicalCallInstructionOrigin> {
    let mut matches = blocks.flat_map(|block| {
        block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
            .filter(move |candidate| candidate.id == instruction)
            .map(move |candidate| (block.id, candidate))
    });
    let (block, row) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(PhysicalCallInstructionOrigin {
        target: row.kind.call_target(),
        block,
        callee: row.kind.machine_callee(),
        operations: row.provenance.operations.clone(),
        edges: row.provenance.edges.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{
        SelectedBlockId, SelectedBlockOrigin, SelectedCallTarget, SelectedInstruction,
        SelectedInstructionKind, SelectedTerminator,
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
        let origin = instruction_origin(blocks.iter(), SelectedInstructionId(3)).unwrap();
        drop(blocks);
        assert_eq!(origin.block, SelectedBlockId(0));
        assert_eq!(origin.callee, MachineId::new(9));
        assert_eq!(origin.edges, [EdgeId::new(7).unwrap()]);
        assert!(origin.operations.is_empty());
    }

    #[test]
    fn absent_or_ambiguous_instruction_does_not_acquire_call_custody() {
        let mut block = completion();
        assert!(instruction_origin([&block].into_iter(), SelectedInstructionId(4)).is_none());
        block
            .instructions
            .push(terminator_instruction(&block.terminator).clone());
        assert!(instruction_origin([&block].into_iter(), SelectedInstructionId(3)).is_none());
    }
}
