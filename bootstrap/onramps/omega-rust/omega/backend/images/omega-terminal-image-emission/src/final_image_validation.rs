//! Exact final-image replay for terminal-Psi artifacts.
//!
//! This module checks import and relocation closure, complete executable-region
//! classification, exact function-span binding, and the final text relocation
//! envelope. It does not write or publish an image.

use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    validate_final_text_relocation_envelope,
};
use omega_terminal_installation_evidence::{
    NativeFuelRuntimeTextEvidence, TerminalNativeFuelTransferRuntimeEvidence,
};
use psi_diagnostics::Diagnostic;

use super::{
    LINUX_X86_SCALAR_EXIT_SHIM_BYTES, TerminalLinuxX86ScalarExitShim, TerminalObjectArtifact,
    ValidatedTerminalNativeFuelArtifact, ValidatedTerminalNativeFuelTransferRuntimeArtifact,
    native_fuel_runtime::replay_terminal_native_fuel_transfer_runtime_artifact,
};

pub(super) fn validate_terminal_image(
    artifact: &TerminalObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
    text_bytes: &[u8],
    scalar_exit_shim: Option<TerminalLinuxX86ScalarExitShim>,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if output.final_image_imports != 0 {
        return Err(Diagnostic::error(
            "terminal-Psi internal-call image unexpectedly retained imports",
        ));
    }
    if output.final_image_relocations != relocations.record_count() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            relocations.record_count()
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
    let expected_region_count = artifact.functions.len() + usize::from(scalar_exit_shim.is_some());
    if compiler_regions.len() != expected_region_count {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            expected_region_count
        )));
    }
    for function in &artifact.functions {
        let symbol = omega_object_file::object_symbol_name(object, function.symbol);
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
    if let Some(shim) = scalar_exit_shim {
        validate_linux_x86_scalar_exit_shim(artifact, object, text_bytes, shim, output)?;
    }
    validate_final_text_relocation_envelope(text_bytes, &output.final_text_bytes, relocations)
}

fn validate_linux_x86_scalar_exit_shim(
    artifact: &TerminalObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    text_bytes: &[u8],
    shim: TerminalLinuxX86ScalarExitShim,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let end = shim
        .text_offset
        .checked_add(shim.byte_count)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry shim range overflows"))?;
    if shim.byte_count != LINUX_X86_SCALAR_EXIT_SHIM_BYTES.len()
        || text_bytes.get(shim.text_offset..end) != Some(&LINUX_X86_SCALAR_EXIT_SHIM_BYTES)
        || shim.relocation_offset != shim.text_offset + 1
        || shim.target_symbol != artifact.entry_function().symbol
    {
        return Err(Diagnostic::error(
            "terminal scalar entry shim does not retain its exact product encoding",
        ));
    }
    let symbol = omega_object_file::object_symbol_name(object, shim.symbol);
    let matching = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| {
            region.origin == FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == symbol
                && region.section_offset == shim.text_offset
                && region.byte_count == shim.byte_count
        })
        .count();
    if matching != 1 {
        return Err(Diagnostic::error(format!(
            "terminal scalar entry shim must bind exactly one final executable region; found {matching}"
        )));
    }

    let expected_entry = output
        .executable_regions
        .text_address
        .checked_add(shim.text_offset as u64)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry address overflows"))?;
    let encoded_entry = output
        .bytes
        .get(24..32)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes);
    if encoded_entry != Some(expected_entry) {
        return Err(Diagnostic::error(
            "ELF entry does not point at the terminal scalar exit shim",
        ));
    }

    let final_shim = output
        .final_text_bytes
        .get(shim.text_offset..end)
        .ok_or_else(|| Diagnostic::error("final image truncates terminal scalar entry shim"))?;
    if final_shim.first() != Some(&0xe8)
        || final_shim.get(5..) != LINUX_X86_SCALAR_EXIT_SHIM_BYTES.get(5..)
    {
        return Err(Diagnostic::error(
            "final terminal scalar entry shim changed outside its call relocation",
        ));
    }
    let displacement = final_shim
        .get(1..5)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_le_bytes)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry call is truncated"))?;
    let instruction_end = output
        .executable_regions
        .text_address
        .checked_add(shim.text_offset as u64 + 5)
        .ok_or_else(|| Diagnostic::error("terminal scalar entry call address overflows"))?;
    let actual_target = instruction_end.checked_add_signed(i64::from(displacement));
    let expected_target = output
        .executable_regions
        .text_address
        .checked_add(artifact.entry_function().text_offset as u64);
    if actual_target != expected_target {
        return Err(Diagnostic::error(
            "terminal scalar entry shim call does not resolve to the semantic entry function",
        ));
    }
    Ok(())
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

pub(super) fn validate_terminal_native_fuel_transfer_runtime_image(
    artifact: &ValidatedTerminalNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
) -> Result<
    (
        CompilerTextValidationEvidence,
        TerminalNativeFuelTransferRuntimeEvidence,
    ),
    Diagnostic,
> {
    let encoding = replay_terminal_native_fuel_transfer_runtime_artifact(artifact)?;
    if output.final_image_imports != 0 {
        return Err(Diagnostic::error(
            "native fuel transfer image unexpectedly retained imports",
        ));
    }
    if output.final_image_relocations != artifact.relocations().record_count() {
        return Err(Diagnostic::error(format!(
            "native fuel transfer image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            artifact.relocations().record_count()
        )));
    }
    if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "native fuel transfer image left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }

    let compiler_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
        .collect::<Vec<_>>();
    let expected_regions = artifact
        .metered_artifact()
        .functions()
        .len()
        .checked_add(2)
        .ok_or_else(|| Diagnostic::error("native fuel transfer region count overflows"))?;
    if compiler_regions.len() != expected_regions {
        return Err(Diagnostic::error(format!(
            "native fuel transfer image retained {} compiler region(s), expected {expected_regions}",
            compiler_regions.len()
        )));
    }
    for (semantic, metered) in artifact
        .metered_artifact()
        .semantic_artifact()
        .functions()
        .iter()
        .zip(artifact.metered_artifact().functions())
    {
        validate_exact_compiler_region(
            &compiler_regions,
            omega_object_file::object_symbol_name(artifact.object(), semantic.symbol),
            metered.text_offset,
            metered.byte_count,
            "metered function",
        )?;
    }
    for (binding, kind) in [
        (artifact.transfer(), "transfer"),
        (artifact.resume(), "resume"),
    ] {
        validate_exact_compiler_region(
            &compiler_regions,
            omega_object_file::object_symbol_name(artifact.object(), binding.symbol()),
            binding.span().text_offset,
            binding.span().byte_count,
            kind,
        )?;
    }

    let compiler_text_validation = validate_final_text_relocation_envelope(
        artifact.text_bytes(),
        &output.final_text_bytes,
        artifact.relocations(),
    )?;
    validate_runtime_rel32_targets(artifact, output, &encoding)?;

    let transfer_text = runtime_text_evidence(artifact, output, artifact.transfer())?;
    let resume_text = runtime_text_evidence(artifact, output, artifact.resume())?;
    let evidence = TerminalNativeFuelTransferRuntimeEvidence::new(
        artifact.plan().clone(),
        transfer_text,
        resume_text,
        encoding.physical_state_footprint().clone(),
        encoding.realized_sponsor_stack_peak_bytes(),
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "native fuel transfer runtime evidence is inconsistent: {error}"
        ))
    })?;
    Ok((compiler_text_validation, evidence))
}

fn validate_exact_compiler_region(
    compiler_regions: &[&omega_image::PlacedExecutableRegion],
    symbol: &str,
    section_offset: usize,
    byte_count: usize,
    kind: &str,
) -> Result<(), Diagnostic> {
    let matching = compiler_regions
        .iter()
        .filter(|region| {
            region.symbol == symbol
                && region.section_offset == section_offset
                && region.byte_count == byte_count
        })
        .count();
    if matching != 1 {
        return Err(Diagnostic::error(format!(
            "native fuel {kind} must bind exactly one final executable region; found {matching}"
        )));
    }
    Ok(())
}

fn validate_runtime_rel32_targets(
    artifact: &ValidatedTerminalNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
    encoding: &omega_terminal_isa_x86_64::X86NativeFuelTransferRuntimeEncoding,
) -> Result<(), Diagnostic> {
    let sponsor_offset = artifact
        .object()
        .layout
        .symbols
        .get(artifact.sponsor_symbol())
        .offset;
    let call_field = artifact
        .transfer()
        .span()
        .text_offset
        .checked_add(encoding.sponsor_call_rel32_field_offset())
        .ok_or_else(|| Diagnostic::error("native fuel sponsor-call field overflows"))?;
    validate_rel32_target(
        &output.final_text_bytes,
        call_field,
        sponsor_offset,
        "sponsor call",
    )?;

    let retry_base_field = artifact
        .resume()
        .span()
        .text_offset
        .checked_add(encoding.retry_text_base_rel32_field_offset())
        .ok_or_else(|| Diagnostic::error("native fuel retry-base field overflows"))?;
    validate_rel32_target(
        &output.final_text_bytes,
        retry_base_field,
        0,
        "retry text base",
    )
}

fn validate_rel32_target(
    final_text: &[u8],
    field_offset: usize,
    expected_target_offset: usize,
    kind: &str,
) -> Result<(), Diagnostic> {
    let field_end = field_offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error(format!("native fuel {kind} field overflows")))?;
    let displacement = final_text
        .get(field_offset..field_end)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(i32::from_le_bytes)
        .ok_or_else(|| Diagnostic::error(format!("native fuel {kind} field is truncated")))?;
    let actual_target = i64::try_from(field_end)
        .ok()
        .and_then(|end| end.checked_add(i64::from(displacement)));
    if actual_target != i64::try_from(expected_target_offset).ok() {
        return Err(Diagnostic::error(format!(
            "native fuel {kind} relocation does not resolve to its exact .text target"
        )));
    }
    Ok(())
}

fn runtime_text_evidence(
    artifact: &ValidatedTerminalNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
    binding: super::TerminalNativeFuelRuntimeEntryBinding,
) -> Result<NativeFuelRuntimeTextEvidence, Diagnostic> {
    let span = binding.span();
    let end = span
        .text_offset
        .checked_add(span.byte_count)
        .ok_or_else(|| Diagnostic::error("native fuel runtime evidence span overflows"))?;
    let unrelocated = artifact
        .text_bytes()
        .get(span.text_offset..end)
        .ok_or_else(|| Diagnostic::error("native fuel unrelocated evidence is truncated"))?;
    let final_bytes = output
        .final_text_bytes
        .get(span.text_offset..end)
        .ok_or_else(|| Diagnostic::error("native fuel final evidence is truncated"))?;
    NativeFuelRuntimeTextEvidence::new(
        binding.identity(),
        span,
        unrelocated.to_vec(),
        final_bytes.to_vec(),
    )
    .map_err(|error| Diagnostic::error(format!("invalid native fuel text evidence: {error}")))
}
