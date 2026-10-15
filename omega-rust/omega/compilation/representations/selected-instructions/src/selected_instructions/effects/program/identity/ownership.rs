use optimization_unit::OwnershipEvent;
use terminal_psi::{StructuralPathSegment, TerminalAffineCleanupAction};

use super::values::encode_ids;
use crate::codec_bytes::encode_len;

pub fn encode_ownership(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
    encode_len(bytes, ownership.len());
    for event in ownership {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.push(1);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::ClaimCompletion(receipts) => {
                bytes.push(2);
                encode_len(bytes, receipts.len());
                for receipt in receipts {
                    bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
                    bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
                    encode_path(bytes, &receipt.path);
                }
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
        encode_projection(bytes, segment);
    }
}

pub fn encode_reference_origin_path(
    bytes: &mut Vec<u8>,
    path: &[terminal_psi::ReferenceOriginProjection],
) {
    encode_len(bytes, path.len());
    for projection in path {
        match projection {
            terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                encode_projection(bytes, segment)
            }
            terminal_psi::ReferenceOriginProjection::Element => bytes.push(9),
        }
    }
}

fn encode_projection(bytes: &mut Vec<u8>, segment: &StructuralPathSegment) {
    match segment {
        StructuralPathSegment::AllElements => bytes.push(10),
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
            crate::codec_bytes::push_encoded(bytes, name.0.to_le_bytes());
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
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
            bytes.push(u8::from(stored_scalar.is_some()));
            if let Some(scalar) = stored_scalar {
                crate::selected_instructions::plan_identity::encode_scalar_type(bytes, *scalar);
            }
        }
    }
}
