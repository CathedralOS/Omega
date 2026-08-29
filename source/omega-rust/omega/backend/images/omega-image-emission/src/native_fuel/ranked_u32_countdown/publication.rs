//! Final-byte replay for the exact ranked native-fuel function.

use omega_object_file::SectionKind;
use psi_diagnostics::Diagnostic;

use super::coordinates;
use crate::{NativeFuelValidationError, ValidatedNativeFuelArtifact};

pub(super) fn replay_metered_final_image(
    artifact: &ValidatedNativeFuelArtifact,
    final_text: &[u8],
) -> Result<(), Diagnostic> {
    let semantic_ranked = artifact
        .semantic_artifact()
        .functions()
        .iter()
        .filter(|function| function.ranked_u32_countdown.is_some())
        .collect::<Vec<_>>();
    let metered_ranked = artifact
        .functions()
        .iter()
        .filter(|function| function.ranked_u32_countdown.is_some())
        .collect::<Vec<_>>();
    if semantic_ranked.is_empty() && metered_ranked.is_empty() {
        return Ok(());
    }
    let ([semantic], [metered]) = (semantic_ranked.as_slice(), metered_ranked.as_slice()) else {
        return Err(Diagnostic::error(
            "ranked-u32 native-fuel final image has missing or mixed ranked custody",
        ));
    };
    if artifact.semantic_artifact().functions().len() != 1
        || artifact.functions().len() != 1
        || artifact.semantic_artifact().entry() != semantic.machine
        || semantic.machine != metered.machine
    {
        return Err(Diagnostic::error(format!(
            "ranked-u32 native-fuel function {} has invalid final-image ownership",
            semantic.machine
        )));
    }

    let source_fuel = artifact
        .semantic_artifact()
        .fuel_attribution()
        .iter()
        .filter(|row| row.machine == semantic.machine)
        .map(|row| row.attribution)
        .collect::<Vec<_>>();
    let expected_record = coordinates::reconstruct(
        artifact.semantic_artifact().target(),
        semantic.bytes(artifact.semantic_artifact()),
        &source_fuel,
    )
    .ok_or_else(|| invalid(semantic.machine, "coordinate reconstruction"))?;
    if metered.ranked_u32_countdown != Some(expected_record) {
        return Err(invalid(semantic.machine, "coordinate custody"));
    }

    let start = metered.text_offset;
    let end = start
        .checked_add(metered.byte_count)
        .ok_or_else(|| invalid(semantic.machine, "text span"))?;
    let admitted = artifact
        .text_bytes()
        .get(start..end)
        .ok_or_else(|| invalid(semantic.machine, "admitted text span"))?;
    let finalized = final_text
        .get(start..end)
        .ok_or_else(|| invalid(semantic.machine, "final text span"))?;
    if admitted != finalized {
        return Err(invalid(semantic.machine, "final byte custody"));
    }
    for (_, relocation) in artifact.relocations().records() {
        let relocation_end = relocation
            .offset
            .checked_add(relocation.byte_width)
            .ok_or_else(|| invalid(semantic.machine, "relocation span"))?;
        if relocation.section == SectionKind::Text
            && relocation.offset < end
            && relocation_end > start
        {
            return Err(invalid(semantic.machine, "relocation isolation"));
        }
    }
    replay_final_charge_fragments(artifact, semantic, metered, &source_fuel, finalized)?;
    if !coordinates::validate_final_branches(
        artifact.semantic_artifact().target(),
        finalized,
        expected_record,
    ) {
        return Err(invalid(semantic.machine, "final branch decoding"));
    }
    Ok(())
}

fn replay_final_charge_fragments(
    artifact: &ValidatedNativeFuelArtifact,
    semantic: &crate::ObjectFunction,
    metered: &crate::ValidatedNativeFuelFunction,
    source_fuel: &[omega_machine_code::NativeFuelAttribution],
    finalized: &[u8],
) -> Result<(), Diagnostic> {
    let machine = semantic.machine;
    if source_fuel.len() != 9
        || artifact.semantic_artifact().fuel_attribution().len() != source_fuel.len()
        || metered.charges.len() != source_fuel.len()
    {
        return Err(invalid(machine, "nine-row fuel closure"));
    }
    let architecture = artifact.semantic_artifact().target().architecture;
    let hot_size = crate::native_fuel::general::hot_charge_byte_count(architecture);
    let cold_size = crate::native_fuel::general::cold_dispatch_byte_count(architecture);
    let semantic_end = semantic
        .byte_count
        .checked_add(
            hot_size
                .checked_mul(source_fuel.len())
                .ok_or_else(|| invalid(machine, "semantic-end coordinate"))?,
        )
        .ok_or_else(|| invalid(machine, "semantic-end coordinate"))?;
    let final_size = semantic_end
        .checked_add(
            cold_size
                .checked_mul(source_fuel.len())
                .ok_or_else(|| invalid(machine, "cold-dispatch coordinate"))?,
        )
        .ok_or_else(|| invalid(machine, "cold-dispatch coordinate"))?;
    if metered.semantic_end_offset != semantic_end || metered.byte_count != final_size {
        return Err(invalid(machine, "metered function extent"));
    }

    for (ordinal, (source, charge)) in source_fuel.iter().zip(&metered.charges).enumerate() {
        let object_source = &artifact.semantic_artifact().fuel_attribution()[ordinal];
        let expected_source_text_offset = semantic
            .text_offset
            .checked_add(source.code_offset)
            .ok_or_else(|| invalid(machine, "source attribution coordinate"))?;
        let charge_offset = source
            .code_offset
            .checked_add(
                hot_size
                    .checked_mul(ordinal)
                    .ok_or_else(|| invalid(machine, "hot-charge coordinate"))?,
            )
            .ok_or_else(|| invalid(machine, "hot-charge coordinate"))?;
        let semantic_offset = charge_offset
            .checked_add(hot_size)
            .ok_or_else(|| invalid(machine, "semantic coordinate"))?;
        let cold_offset = semantic_end
            .checked_add(
                cold_size
                    .checked_mul(ordinal)
                    .ok_or_else(|| invalid(machine, "cold-dispatch coordinate"))?,
            )
            .ok_or_else(|| invalid(machine, "cold-dispatch coordinate"))?;
        if object_source.machine != machine
            || object_source.attribution != *source
            || object_source.text_offset != expected_source_text_offset
            || charge.attribution != *source
            || charge.charge_code_offset != charge_offset
            || charge.charge_byte_count != hot_size
            || charge.semantic_code_offset != semantic_offset
            || charge.cold_dispatch_code_offset != cold_offset
            || charge.cold_dispatch_byte_count != cold_size
        {
            return Err(invalid(machine, "source-to-charge correspondence"));
        }
        let hot = fragment(finalized, charge_offset, hot_size)
            .ok_or_else(|| invalid(machine, "final hot-charge span"))?;
        let cold = fragment(finalized, cold_offset, cold_size)
            .ok_or_else(|| invalid(machine, "final cold-dispatch span"))?;
        crate::native_fuel::general::validate_charge(
            architecture,
            &artifact.target_policy(),
            *source,
            charge_offset,
            cold_offset,
            hot,
        )
        .map_err(|error| fragment_error(machine, "hot-charge decoding", error))?;
        let retry_text_offset = metered
            .text_offset
            .checked_add(charge_offset)
            .and_then(|offset| u64::try_from(offset).ok())
            .ok_or_else(|| invalid(machine, "retry coordinate"))?;
        crate::native_fuel::general::validate_cold_dispatch(
            architecture,
            &artifact.target_policy(),
            *source,
            retry_text_offset,
            cold,
        )
        .map_err(|error| fragment_error(machine, "cold-dispatch decoding", error))?;
    }
    Ok(())
}

fn fragment(bytes: &[u8], offset: usize, count: usize) -> Option<&[u8]> {
    bytes.get(offset..offset.checked_add(count)?)
}

fn fragment_error(
    machine: psi_core::MachineId,
    custody: &str,
    error: NativeFuelValidationError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "ranked-u32 native-fuel function {machine} failed {custody} replay: {error}"
    ))
}

fn invalid(machine: psi_core::MachineId, custody: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "ranked-u32 native-fuel function {machine} failed {custody} replay"
    ))
}
