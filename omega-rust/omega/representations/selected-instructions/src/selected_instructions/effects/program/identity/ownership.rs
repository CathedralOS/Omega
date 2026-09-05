use optimization_unit::OwnershipEvent;
use terminal_psi::{StructuralPathSegment, TerminalAffineCleanupAction};

use super::values::{encode_ids, encode_len};

pub fn encode_ownership(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
    encode_len(bytes, ownership.len());
    for event in ownership {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.push(1);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::ClaimCompletion(claims) => {
                bytes.push(2);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::Cleanup(actions) => {
                bytes.push(3);
                encode_len(bytes, actions.len());
                for action in actions {
                    encode_cleanup(bytes, action);
                }
            }
            OwnershipEvent::StructuralReturn(claims) => {
                bytes.push(4);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::CrashFrontier(claims) => {
                bytes.push(5);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
        }
    }
}

fn encode_cleanup(bytes: &mut Vec<u8>, action: &TerminalAffineCleanupAction) {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            bytes.extend_from_slice(&discard.place.get().to_le_bytes());
            encode_path(bytes, &discard.path);
            bytes.extend_from_slice(&discard.structural_type.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(3);
            bytes.extend_from_slice(&cleanup.place.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            match cleanup.cleanup_receiver {
                None => bytes.push(0),
                Some(place) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&place.get().to_le_bytes());
                }
            }
            encode_ids(
                bytes,
                cleanup.requirement_obligations.iter().map(|id| id.get()),
            );
        }
    }
}

fn encode_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_len(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(name) => {
                bytes.push(1);
                encode_len(bytes, name.len());
                bytes.extend_from_slice(name.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
}
