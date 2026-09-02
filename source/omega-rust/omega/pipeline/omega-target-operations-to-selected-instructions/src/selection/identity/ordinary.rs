//! Canonical identity encoding for ordinary selected control.

use super::*;

pub(super) fn encode_terminator(bytes: &mut Vec<u8>, terminator: &SelectedTerminator) {
    match terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => {
            bytes.push(0);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_nonzero);
            encode_successor(bytes, when_zero);
        }
        SelectedTerminator::Return {
            instruction,
            psi_return_edge,
        } => {
            bytes.push(1);
            encode_instruction(bytes, instruction);
            bytes.extend_from_slice(&psi_return_edge.get().to_le_bytes());
        }
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => {
            bytes.push(2);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_less);
            encode_successor(bytes, when_not_less);
        }
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => {
            bytes.push(3);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_less);
            encode_successor(bytes, when_not_less);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use psi_core::{BlockId, EdgeId};

    fn instruction(kind: SelectedInstructionKind) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(1),
            kind,
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 7,
            },
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        }
    }

    fn successor(edge: u64, block: u32, source_target: u64) -> SelectedSuccessor {
        SelectedSuccessor {
            psi_edge: EdgeId::new(edge).expect("edge"),
            block: SelectedBlockId(block),
            source_target: BlockId::new(source_target).expect("block"),
            bindings: Vec::new(),
            fuel: Vec::new(),
        }
    }

    #[test]
    fn signed_less_than_identity_retains_predicate_and_successor_order() {
        let terminator = SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: instruction(SelectedInstructionKind::ConditionalBranchI64LessThan),
            when_less: successor(2, 1, 3),
            when_not_less: successor(4, 2, 5),
        };
        let mut canonical = Vec::new();
        encode_terminator(&mut canonical, &terminator);

        let SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: signed_instruction,
            when_less,
            when_not_less,
        } = terminator
        else {
            unreachable!()
        };
        let swapped = SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: signed_instruction,
            when_less: when_not_less,
            when_not_less: when_less,
        };
        let mut swapped_bytes = Vec::new();
        encode_terminator(&mut swapped_bytes, &swapped);
        assert_ne!(canonical, swapped_bytes);

        let unsigned = SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: instruction(SelectedInstructionKind::ConditionalBranchU64LessThan),
            when_less: successor(2, 1, 3),
            when_not_less: successor(4, 2, 5),
        };
        let mut unsigned_bytes = Vec::new();
        encode_terminator(&mut unsigned_bytes, &unsigned);
        assert_ne!(canonical, unsigned_bytes);
    }
}
