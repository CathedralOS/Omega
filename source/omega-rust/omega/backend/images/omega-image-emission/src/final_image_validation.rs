//! Exact final-image replay for terminal-Psi artifacts.
//!
//! This module checks import and relocation closure, complete executable-region
//! classification, exact function-span binding, and the final text relocation
//! envelope. It does not write or publish an image.

use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    validate_final_text_relocation_envelope,
};
use omega_installation_evidence::{
    NativeFuelRuntimeTextEvidence, NativeFuelTransferRuntimeEvidence,
};
use psi_diagnostics::Diagnostic;

use super::{
    LINUX_X86_SCALAR_EXIT_SHIM_BYTES, LinuxX86ScalarExitShim, ObjectArtifact,
    ValidatedNativeFuelArtifact, ValidatedNativeFuelTransferRuntimeArtifact,
    native_fuel_runtime::{
        NativeFuelTransferRuntimeEncoding, replay_terminal_native_fuel_transfer_runtime_artifact,
    },
};

pub(super) fn validate_terminal_image(
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    relocations: &omega_object_file::RelocationPlan,
    text_bytes: &[u8],
    scalar_exit_shim: Option<LinuxX86ScalarExitShim>,
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
    artifact: &ObjectArtifact,
    object: &omega_object_file::ObjectPlan,
    text_bytes: &[u8],
    shim: LinuxX86ScalarExitShim,
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
    artifact: &ValidatedNativeFuelArtifact,
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
    let validation = validate_final_text_relocation_envelope(
        artifact.text_bytes(),
        &output.final_text_bytes,
        artifact.relocations(),
    )?;
    super::native_fuel::replay_ranked_native_fuel_final_image(artifact, &output.final_text_bytes)?;
    Ok(validation)
}

pub(super) fn validate_terminal_native_fuel_transfer_runtime_image(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
) -> Result<
    (
        CompilerTextValidationEvidence,
        NativeFuelTransferRuntimeEvidence,
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
    super::native_fuel::replay_ranked_native_fuel_final_image(
        artifact.metered_artifact(),
        &output.final_text_bytes,
    )?;
    validate_runtime_relocation_targets(artifact, output, &encoding)?;

    let transfer_text = runtime_text_evidence(artifact, output, artifact.transfer())?;
    let resume_text = runtime_text_evidence(artifact, output, artifact.resume())?;
    let evidence = NativeFuelTransferRuntimeEvidence::new(
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

fn validate_runtime_relocation_targets(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
    encoding: &NativeFuelTransferRuntimeEncoding,
) -> Result<(), Diagnostic> {
    match encoding {
        NativeFuelTransferRuntimeEncoding::X86_64(encoding) => {
            validate_runtime_x86_rel32_targets(artifact, output, encoding)
        }
        NativeFuelTransferRuntimeEncoding::Aarch64(encoding) => {
            validate_runtime_aarch64_targets(artifact, output, encoding)
        }
    }
}

fn validate_runtime_x86_rel32_targets(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
    encoding: &omega_isa_x86_64::X86NativeFuelTransferRuntimeEncoding,
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

fn validate_runtime_aarch64_targets(
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
    encoding: &omega_isa_aarch64::Aarch64NativeFuelTransferRuntimeEncoding,
) -> Result<(), Diagnostic> {
    let text_address = output.executable_regions.text_address;
    let sponsor_offset = artifact
        .object()
        .layout
        .symbols
        .get(artifact.sponsor_symbol())
        .offset;
    let sponsor_address = text_address
        .checked_add(u64::try_from(sponsor_offset).map_err(|_| {
            Diagnostic::error("native fuel AArch64 sponsor offset is not addressable")
        })?)
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 sponsor address overflows"))?;
    let branch_offset = artifact
        .transfer()
        .span()
        .text_offset
        .checked_add(encoding.sponsor_call_branch26_offset())
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 sponsor branch overflows"))?;
    validate_aarch64_branch26_target(
        &output.final_text_bytes,
        text_address,
        branch_offset,
        sponsor_address,
    )?;

    let page21_offset = artifact
        .resume()
        .span()
        .text_offset
        .checked_add(encoding.retry_text_page21_offset())
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 retry Page21 field overflows"))?;
    let page_offset12_offset = artifact
        .resume()
        .span()
        .text_offset
        .checked_add(encoding.retry_text_page_offset12_offset())
        .ok_or_else(|| {
            Diagnostic::error("native fuel AArch64 retry PageOffset12 field overflows")
        })?;
    validate_aarch64_page_address_target(
        &output.final_text_bytes,
        text_address,
        page21_offset,
        page_offset12_offset,
        text_address,
    )
}

fn validate_aarch64_branch26_target(
    final_text: &[u8],
    text_address: u64,
    instruction_offset: usize,
    expected_target_address: u64,
) -> Result<(), Diagnostic> {
    let instruction = read_aarch64_instruction(final_text, instruction_offset, "sponsor branch")?;
    if instruction & 0xfc00_0000 != 0x9400_0000 {
        return Err(Diagnostic::error(
            "native fuel AArch64 sponsor relocation no longer names a BL instruction",
        ));
    }
    let instruction_address = text_address
        .checked_add(u64::try_from(instruction_offset).map_err(|_| {
            Diagnostic::error("native fuel AArch64 sponsor branch offset is not addressable")
        })?)
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 branch address overflows"))?;
    let delta = sign_extend(u64::from(instruction & 0x03ff_ffff), 26)
        .checked_mul(4)
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 branch delta overflows"))?;
    if instruction_address.checked_add_signed(delta) != Some(expected_target_address) {
        return Err(Diagnostic::error(
            "native fuel AArch64 sponsor relocation does not resolve to its exact .text target",
        ));
    }
    Ok(())
}

fn validate_aarch64_page_address_target(
    final_text: &[u8],
    text_address: u64,
    page21_offset: usize,
    page_offset12_offset: usize,
    expected_target_address: u64,
) -> Result<(), Diagnostic> {
    let adrp = read_aarch64_instruction(final_text, page21_offset, "retry Page21")?;
    let add = read_aarch64_instruction(final_text, page_offset12_offset, "retry PageOffset12")?;
    if adrp & 0x9f00_001f != 0x9000_0010 || add & 0xffc0_03ff != 0x9100_0210 {
        return Err(Diagnostic::error(
            "native fuel AArch64 retry-base relocation pair changed instruction shape",
        ));
    }
    let instruction_address = text_address
        .checked_add(u64::try_from(page21_offset).map_err(|_| {
            Diagnostic::error("native fuel AArch64 Page21 offset is not addressable")
        })?)
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 Page21 address overflows"))?;
    let immediate = u64::from(((adrp >> 29) & 0b11) | (((adrp >> 5) & 0x7ffff) << 2));
    let page_delta = sign_extend(immediate, 21)
        .checked_mul(4096)
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 Page21 delta overflows"))?;
    let target_page = (instruction_address & !0xfff)
        .checked_add_signed(page_delta)
        .ok_or_else(|| Diagnostic::error("native fuel AArch64 retry page address overflows"))?;
    let page_offset = u64::from((add >> 10) & 0xfff);
    if target_page.checked_add(page_offset) != Some(expected_target_address) {
        return Err(Diagnostic::error(
            "native fuel AArch64 retry relocation pair does not resolve to the exact .text base",
        ));
    }
    Ok(())
}

fn read_aarch64_instruction(
    final_text: &[u8],
    offset: usize,
    kind: &str,
) -> Result<u32, Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error(format!("native fuel AArch64 {kind} overflows")))?;
    final_text
        .get(offset..end)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| Diagnostic::error(format!("native fuel AArch64 {kind} is truncated")))
}

fn sign_extend(value: u64, bits: u32) -> i64 {
    ((value << (64 - bits)) as i64) >> (64 - bits)
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
    artifact: &ValidatedNativeFuelTransferRuntimeArtifact,
    output: &EmittedImageOutput,
    binding: super::NativeFuelRuntimeEntryBinding,
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

#[cfg(test)]
mod native_fuel_runtime_target_tests {
    use super::*;

    #[test]
    fn aarch64_branch26_replay_requires_the_exact_sponsor_target() {
        let text_address = 0x401000;
        let instruction_offset = 0x100;
        let sponsor_address = 0x401080;
        let mut text = vec![0; instruction_offset + 4];
        // BL from 0x401100 back 128 bytes to 0x401080.
        text[instruction_offset..instruction_offset + 4]
            .copy_from_slice(&0x97ff_ffe0_u32.to_le_bytes());
        validate_aarch64_branch26_target(&text, text_address, instruction_offset, sponsor_address)
            .expect("exact Branch26 target");

        assert!(
            validate_aarch64_branch26_target(
                &text,
                text_address,
                instruction_offset,
                sponsor_address + 4,
            )
            .is_err()
        );
    }

    #[test]
    fn aarch64_page_pair_replay_requires_the_exact_text_base() {
        let text_address = 0x401234;
        let mut text = vec![0; 8];
        text[0..4].copy_from_slice(&0x9000_0010_u32.to_le_bytes());
        text[4..8].copy_from_slice(&0x9108_d210_u32.to_le_bytes());
        validate_aarch64_page_address_target(&text, text_address, 0, 4, text_address)
            .expect("exact Page21/PageOffset12 target");

        text[4..8].copy_from_slice(&0x9108_d610_u32.to_le_bytes());
        assert!(
            validate_aarch64_page_address_target(&text, text_address, 0, 4, text_address).is_err()
        );
    }
}
