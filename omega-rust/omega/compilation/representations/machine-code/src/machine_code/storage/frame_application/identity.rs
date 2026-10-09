use sha2::{Digest, Sha256};

use super::{FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity};

const SCHEMA: &[u8] = b"omega.function-fragment-frame-application.v3";

pub fn function_fragment_frame_application_identity(
    application: &FunctionFragmentFrameApplication,
) -> FunctionFragmentFrameApplicationIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(SCHEMA);
    hasher.raw(&application.source_fragment_manifest.bytes());
    hasher.raw(&application.source_fragments.bytes());
    hasher.raw(&application.frame_protocol.bytes());
    hasher.u64(application.functions.len() as u64);
    for row in &application.functions {
        hasher.raw(&row.machine.get().to_le_bytes());
        hasher.raw(&row.prologue_function_offset.to_le_bytes());
        hasher.raw(&row.prologue_byte_count.to_le_bytes());
        hasher.u64(row.epilogues.len() as u64);
        for epilogue in &row.epilogues {
            hasher.raw(&epilogue.block.0.to_le_bytes());
            hasher.raw(&epilogue.return_instruction.0.to_le_bytes());
            hasher.raw(&epilogue.psi_return_edge.get().to_le_bytes());
            hasher.raw(&epilogue.function_offset.to_le_bytes());
            hasher.raw(&epilogue.byte_count.to_le_bytes());
        }
    }
    hasher.raw(&application.fragments.identity.bytes());
    FunctionFragmentFrameApplicationIdentity::from_bytes(hasher.finish())
}
