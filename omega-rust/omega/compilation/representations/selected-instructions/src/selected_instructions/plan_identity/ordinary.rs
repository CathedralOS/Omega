//! Canonical identity encoding for ordinary selected control.
use super::encode_instruction;
use super::encode_successor;
use crate::SelectedTerminator;

pub(super) fn encode_terminator(bytes: &mut Vec<u8>, terminator: &SelectedTerminator) {
    match terminator {
        SelectedTerminator::Crash {
            instruction,
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            bytes.push(6);
            encode_instruction(bytes, instruction);
            crate::codec_bytes::push_encoded(bytes, psi_edge.get().to_le_bytes());
            bytes.push(match cause {
                terminal_psi::CrashCause::Trap => 1,
                terminal_psi::CrashCause::Abort => 2,
            });
            crate::codec_bytes::encode_len(bytes, site_guard.len());
            for predicate in site_guard {
                let encoded =
                    terminal_codec::canonical_proposition_order_key(predicate.proposition())
                        .expect("validated crash predicate");
                crate::codec_bytes::encode_len(bytes, encoded.len());
                bytes.extend_from_slice(&encoded);
            }
            crate::codec_bytes::encode_len(bytes, frontier_lower_bound.len());
            for claim in frontier_lower_bound {
                crate::codec_bytes::push_encoded(bytes, claim.get().to_le_bytes());
            }
        }

        SelectedTerminator::HostedExitProcess {
            instruction,
            completion_edge,
        } => {
            bytes.push(5);
            encode_instruction(bytes, instruction);
            crate::codec_bytes::push_encoded(bytes, completion_edge.get().to_le_bytes());
        }
        SelectedTerminator::Jump {
            instruction,
            successor,
        } => {
            bytes.push(4);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, successor);
        }
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
            crate::codec_bytes::push_encoded(bytes, psi_return_edge.get().to_le_bytes());
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
    use super::encode_terminator;
    use crate::SelectedTerminator;
    use crate::{
        SelectedBlockId, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance, SelectedSuccessor,
    };
    use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use semantic_vocabulary::{BlockId, EdgeId};

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
            structural_case: None,
            role: crate::SelectedSuccessorRole::Semantic,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(edge).expect("edge"),
            block: SelectedBlockId(block),
            source_target: BlockId::new(source_target).expect("block"),
            bindings: Vec::new(),
            fuel: Vec::new(),
        }
    }

    #[test]
    fn structural_snapshot_identity_retains_every_selector_coordinate() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId,
        };
        let mut original = successor(2, 1, 3);
        original
            .structural_bindings
            .push(crate::SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(4).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(5).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                },
                transport: crate::SelectedStructuralTransport::Unused,
                source_indices: vec![legalized_operations::LegalizedRuntimeIndexOperand {
                    operand: abstract_operations::AbstractResult {
                        value: ValueId::new(6).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                    },
                    stride: 16,
                    extent: Some(7),
                    obligation: ObligationId::new(8).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [9; 32],
                    ),
                }],
            });
        let encode = |value: &SelectedSuccessor| {
            let mut bytes = Vec::new();
            super::encode_successor(&mut bytes, value);
            bytes
        };
        let canonical = encode(&original);
        for mutation in 0..7 {
            let mut changed = original.clone();
            let index = &mut changed.structural_bindings[0].source_indices[0];
            match mutation {
                0 => index.operand.value = ValueId::new(10).unwrap(),
                1 => {
                    index.operand.scalar_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
                }
                2 => index.stride += 1,
                3 => index.extent = None,
                4 => index.obligation = ObligationId::new(11).unwrap(),
                5 => {
                    index.accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([12; 32])
                }
                _ => changed.structural_bindings[0].source_indices.clear(),
            }
            assert_ne!(encode(&changed), canonical, "selector mutation {mutation}");
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
