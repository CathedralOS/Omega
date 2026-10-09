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
            OwnershipEvent::ExternalCompletionFrontier(claims) => {
                bytes.push(6);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
        }
    }
}

pub fn encode_cleanup(bytes: &mut Vec<u8>, action: &TerminalAffineCleanupAction) {
    match action {
        TerminalAffineCleanupAction::RetireHome(place) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, discard.place.get().to_le_bytes());
            encode_path(bytes, &discard.path);
            crate::codec_bytes::push_encoded(bytes, discard.structural_type.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(if cleanup.retains_descendants { 5 } else { 3 });
            crate::codec_bytes::push_encoded(bytes, cleanup.place.get().to_le_bytes());
            encode_path(bytes, &cleanup.path);
            crate::codec_bytes::push_encoded(bytes, cleanup.structural_type.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, cleanup.cleanup_machine.get().to_le_bytes());
            encode_ids(
                bytes,
                cleanup.consumed_claims.iter().map(|claim| claim.get()),
            );
            match cleanup.cleanup_receiver {
                None => bytes.push(0),
                Some(place) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                }
            }
            encode_ids(
                bytes,
                cleanup.requirement_obligations.iter().map(|id| id.get()),
            );
        }
    }
}

pub fn encode_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_len(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Case(case) => {
                bytes.push(7);
                crate::codec_bytes::push_encoded(bytes, case.get().to_le_bytes());
            }
            StructuralPathSegment::Referent => bytes.push(3),
            StructuralPathSegment::FixedByteRange { start, end } => {
                bytes.push(4);
                crate::codec_bytes::push_encoded(bytes, start.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, end.to_le_bytes());
            }
            StructuralPathSegment::Field(name) => {
                bytes.push(1);
                encode_len(bytes, name.len());
                bytes.extend_from_slice(name.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, index.to_le_bytes());
            }
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                bytes.push(6);
                crate::codec_bytes::push_encoded(bytes, index.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
            }
        }
    }
}

