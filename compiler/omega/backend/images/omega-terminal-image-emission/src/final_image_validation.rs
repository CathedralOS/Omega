//! Exact final-image replay for terminal-Psi artifacts.
//!
//! This module checks import and relocation closure, complete executable-region
//! classification, exact function-span binding, and the final text relocation
//! envelope. It does not write or publish an image.

use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    validate_final_text_relocation_envelope,
};
use psi_diagnostics::Diagnostic;

use super::{TerminalObjectArtifact, ValidatedTerminalNativeFuelArtifact};

pub(super) fn validate_terminal_image(
    artifact: &TerminalObjectArtifact,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if output.final_image_imports != 0 {
        return Err(Diagnostic::error(
            "terminal-Psi internal-call image unexpectedly retained imports",
        ));
    }
    if output.final_image_relocations != artifact.relocations.record_count() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            artifact.relocations.record_count()
        )));
    }
    if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi executable inventory left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    let compiler_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
        .collect::<Vec<_>>();
    if compiler_regions.len() != artifact.functions.len() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            artifact.functions.len()
        )));
    }
    for function in &artifact.functions {
        let symbol = omega_object_file::object_symbol_name(&artifact.object, function.symbol);
        let matching = compiler_regions
            .iter()
            .filter(|region| {
                region.symbol == symbol
                    && region.section_offset == function.text_offset
                    && region.byte_count == function.byte_count
            })
            .count();
        if matching != 1 {
            return Err(Diagnostic::error(format!(
                "terminal-Psi function {} must bind exactly one final executable region; found {matching}",
                function.machine
            )));
        }
    }
    validate_final_text_relocation_envelope(
        &artifact.text_bytes,
        &output.final_text_bytes,
        &artifact.relocations,
    )
}

pub(super) fn validate_terminal_native_fuel_image(
    artifact: &ValidatedTerminalNativeFuelArtifact,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if output.final_image_imports != 0 {
        return Err(Diagnostic::error(
            "metered terminal-Psi image unexpectedly retained imports",
        ));
    }
    if output.final_image_relocations != artifact.relocations().record_count() {
        return Err(Diagnostic::error(format!(
            "metered terminal-Psi image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            artifact.relocations().record_count()
        )));
    }
    if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "metered terminal-Psi executable inventory left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    let compiler_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
        .collect::<Vec<_>>();
    if compiler_regions.len() != artifact.functions().len() {
        return Err(Diagnostic::error(format!(
            "metered terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            artifact.functions().len()
        )));
    }
    for (semantic_function, metered_function) in artifact
        .semantic_artifact()
        .functions()
        .iter()
        .zip(artifact.functions())
    {
        let symbol =
            omega_object_file::object_symbol_name(artifact.object(), semantic_function.symbol);
        let matching = compiler_regions
            .iter()
            .filter(|region| {
                region.symbol == symbol
                    && region.section_offset == metered_function.text_offset
                    && region.byte_count == metered_function.byte_count
            })
            .count();
        if matching != 1 {
            return Err(Diagnostic::error(format!(
                "metered terminal-Psi function {} must bind exactly one final executable region; found {matching}",
                metered_function.machine
            )));
        }
    }
    validate_final_text_relocation_envelope(
        artifact.text_bytes(),
        &output.final_text_bytes,
        artifact.relocations(),
    )
}
