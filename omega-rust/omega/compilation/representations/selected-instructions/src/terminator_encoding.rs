/*
  Canonical byte encoding of `SelectedTerminator` arms and fields.

  Both the plan identity and the fixed-view-copy recovery artifact write the
  same ten terminator arms, tags, and field order, while supplying their own
  `encode_instruction` and `encode_successor` callbacks.
*/

use crate::{SelectedInstruction, SelectedSuccessor, SelectedTerminator};

pub fn encode_terminator(
    bytes: &mut Vec<u8>,
    terminator: &SelectedTerminator,
    encode_instruction: impl Fn(&mut Vec<u8>, &SelectedInstruction),
    encode_successor: impl Fn(&mut Vec<u8>, &SelectedSuccessor),
) {
    match terminator {
        SelectedTerminator::OperationInvokeNoNormal {
            instruction,
            operation,
        } => {
            bytes.push(9);
            encode_instruction(bytes, instruction);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        SelectedTerminator::InvokeNoNormal {
            instruction,
            psi_edge,
        } => {
            bytes.push(8);
            encode_instruction(bytes, instruction);
            crate::codec_bytes::push_encoded(bytes, psi_edge.get().to_le_bytes());
        }
        SelectedTerminator::OperationTrap {
            instruction,
            operation,
        } => {
            bytes.push(7);
            encode_instruction(bytes, instruction);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
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
