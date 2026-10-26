//! Shared canonical-identity byte prefixes and occurrence-identity tails:
//! the terminal-identity prefix plus the machine/operation/site/boundary
//! append sequences both the projection and the evidence-derivation halves
//! write into their canonical preimages.

use super::canonical_usize;

pub(super) fn terminal_identity_bytes(terminal: terminal_psi::TerminalPsiIdentity) -> Vec<u8> {
    let mut canonical = Vec::with_capacity(40);
    crate::codec_bytes::push_encoded(
        &mut canonical,
        terminal.vocabulary_marker.get().to_le_bytes(),
    );
    canonical.extend_from_slice(terminal.program_fingerprint.as_bytes());
    canonical
}

pub(super) fn append_operator_occurrence_identity(
    canonical: &mut Vec<u8>,
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
    operation_ordinal: usize,
) {
    crate::codec_bytes::push_encoded(canonical, machine.get().to_le_bytes());
    crate::codec_bytes::push_encoded(canonical, operation.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
}

pub(super) fn append_boundary_occurrence_identity(
    canonical: &mut Vec<u8>,
    machine: semantic_vocabulary::MachineId,
    site: impl Into<semantic_vocabulary::InvocationSite>,
    boundary: semantic_vocabulary::BoundaryMachineId,
    operation_ordinal: usize,
) {
    crate::codec_bytes::push_encoded(canonical, machine.get().to_le_bytes());
    match site.into() {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            canonical.push(0);
            crate::codec_bytes::push_encoded(canonical, operation.get().to_le_bytes());
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
            canonical.push(1);
            crate::codec_bytes::push_encoded(canonical, edge.get().to_le_bytes());
        }
    }
    crate::codec_bytes::push_encoded(canonical, boundary.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
}
