//! Reporting projection of replayed selected fragments and applied frame bytes.
//!
//! The caller first validates the retained object/source relationship and final
//! image, then passes the freshly derived text validation here. Neither this
//! summary nor its compact coordinates substitute for those validations.

use diagnostics::Diagnostic;
use image::{
    CompilerFunctionValidationEvidence, CompilerTextValidationEvidence, EmittedImageOutput,
};

use crate::ObjectArtifact;

pub(crate) fn summarize(
    artifact: &ObjectArtifact,
    output: &EmittedImageOutput,
    text_validation: &CompilerTextValidationEvidence,
) -> Result<Option<CompilerFunctionValidationEvidence>, Diagnostic> {
    let Some(replay) = &artifact.fragment_replay else {
        // Mechanical object/image APIs do not carry the compiler's selected
        // source. They must not acquire compiler publication evidence here.
        return Ok(None);
    };
    let applied = replay.0.source().source();
    let manifest = applied.source().manifest().record();
    let frame = applied.application();
    let mut prologue_bytes = 0usize;
    let mut epilogue_bytes = 0usize;
    for function in &frame.functions {
        prologue_bytes = prologue_bytes
            .checked_add(host(function.prologue_byte_count)?)
            .ok_or_else(overflow)?;
        for epilogue in &function.epilogues {
            epilogue_bytes = epilogue_bytes
                .checked_add(host(epilogue.byte_count)?)
                .ok_or_else(overflow)?;
        }
    }
    Ok(Some(CompilerFunctionValidationEvidence {
        function_count: host(manifest.statistics.functions)?,
        instruction_count: host(manifest.statistics.instruction_spans)?,
        zero_width_instruction_count: host(manifest.statistics.zero_byte_instruction_spans)?,
        frame_prologue_byte_count: prologue_bytes,
        frame_epilogue_byte_count: epilogue_bytes,
        fragment_manifest_report_fingerprint: compact(manifest.identity.bytes()),
        frame_application_report_fingerprint: compact(frame.identity.bytes()),
        // Current common publication has no separate boundary entry-footprint
        // mutation. Provider and boundary semantics remain in the replayed
        // source; this is not a claim that the source has no effects.
        boundary_contract_report_fingerprint: None,
        final_region_binding_report_fingerprint: output
            .executable_regions
            .inventory_report_fingerprint,
        final_text_validation_report_fingerprint: text_validation.derivation_report_fingerprint,
    }))
}

fn compact(identity: [u8; 32]) -> u64 {
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&identity[..8]);
    u64::from_le_bytes(prefix)
}

fn host(value: u64) -> Result<usize, Diagnostic> {
    usize::try_from(value).map_err(|_| overflow())
}

fn overflow() -> Diagnostic {
    Diagnostic::error("compiler function validation report count exceeds supported size")
}
