use sha2::{Digest, Sha256};

use super::{FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity};

const SCHEMA: &[u8] = b"omega.function-fragment-frame-application.v2";

pub fn function_fragment_frame_application_identity(
    application: &FunctionFragmentFrameApplication,
) -> FunctionFragmentFrameApplicationIdentity {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA);
    hasher.update(application.source_fragment_manifest.bytes());
    hasher.update(application.source_fragments.bytes());
    hasher.update(application.frame_protocol.bytes());
    hasher.update((application.functions.len() as u64).to_le_bytes());
    for row in &application.functions {
        hasher.update(row.machine.get().to_le_bytes());
        hasher.update(row.prologue_function_offset.to_le_bytes());
        hasher.update(row.prologue_byte_count.to_le_bytes());
        hasher.update((row.epilogues.len() as u64).to_le_bytes());
        for epilogue in &row.epilogues {
            hasher.update(epilogue.block.0.to_le_bytes());
            hasher.update(epilogue.return_instruction.0.to_le_bytes());
            hasher.update(epilogue.psi_return_edge.get().to_le_bytes());
            hasher.update(epilogue.function_offset.to_le_bytes());
            hasher.update(epilogue.byte_count.to_le_bytes());
        }
    }
    hasher.update(application.fragments.identity.bytes());
    FunctionFragmentFrameApplicationIdentity::from_bytes(hasher.finalize().into())
}
