//! Bind each checked entry to its own final region. A stored callback does not
//! add its future effects to its installer's synchronous state promise.
use crate::ObjectArtifact;
use crate::entries::hosted_unit_entry::{EntryShim, EntrySurface};
use diagnostics::Diagnostic;
use image::{EmittedImageOutput, FinalExecutableRegionOrigin, FinalImage};

pub(crate) fn attach(
    artifact: &ObjectArtifact,
    image: &mut FinalImage,
    shim: Option<&EntryShim>,
) -> Result<(), Diagnostic> {
    for private in artifact.private_functions() {
        let (handle, symbol) =
            object_file::object_function_symbol(artifact.object(), private.identity).ok_or_else(
                || Diagnostic::error("entry state evidence lost its exact private identity"),
            )?;
        if handle != private.function.symbol
            || symbol.offset != private.function.text_offset
            || symbol.size != private.function.byte_count
        {
            return Err(Diagnostic::error(
                "entry state evidence changed its exact private span",
            ));
        }
        let offset = symbol.offset;
        let size = symbol.size;
        let region = image
            .executable_regions
            .iter_mut()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::CompilerFunction
                    && region.section_offset == offset
                    && region.byte_count == size
            })
            .ok_or_else(|| {
                Diagnostic::error("entry state evidence lost its exact private region")
            })?;
        region.entry_effects = Some(private.entry_effects.clone());
        if let Some(bridge) = &private.entry_bridge {
            region.footprint = Some(bridge.generated().effects.footprint().clone());
        }
    }
    if let Some(shim) = shim
        && shim.surface == EntrySurface::Physical
    {
        let (_, offset) = shim.symbol_offset();
        let (generated, effects) =
            crate::entries::physical_entry::generated_state_effects(artifact).ok_or_else(|| {
                Diagnostic::error("physical entry has no generated state evidence")
            })?;
        let region = image
            .executable_regions
            .iter_mut()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::CompilerFunction
                    && region.section_offset == offset
            })
            .ok_or_else(|| {
                Diagnostic::error("physical entry has no exact state-evidence region")
            })?;
        region.footprint = Some(generated.effects.footprint().clone());
        region.entry_effects = Some(effects.clone());
    }
    Ok(())
}

pub(crate) fn validate(
    artifact: &ObjectArtifact,
    output: &EmittedImageOutput,
    shim: Option<&EntryShim>,
) -> Result<(), Diagnostic> {
    for private in artifact.private_functions() {
        let region = output
            .executable_regions
            .regions
            .iter()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::CompilerFunction
                    && region.section_offset == private.function.text_offset
                    && region.byte_count == private.function.byte_count
            })
            .ok_or_else(|| {
                Diagnostic::error("final private entry state has no exact placed region")
            })?;
        if region.entry_effects.as_ref() != Some(&private.entry_effects) {
            return Err(Diagnostic::error(
                "final private entry state differs from its checked body and plan",
            ));
        }
        if let Some(bridge) = &private.entry_bridge {
            let body = private.body.entry_function();
            let binding = private
                .symbol_bindings
                .iter()
                .find(|(_, binding)| binding.local == body.symbol)
                .ok_or_else(|| Diagnostic::error("entry body call lost its retained body symbol"))?
                .1;
            let symbols = &artifact.object().layout.symbols;
            if !symbols.is_valid(binding.linked) {
                return Err(Diagnostic::error("entry body call has a stale body symbol"));
            }
            let symbol = symbols.get(binding.linked);
            if symbol.section != object_file::SymbolSection::Section(object_file::SectionKind::Text)
                || symbol.kind != object_file::SymbolKind::Function
                || Some(symbol.offset) != private.text_offset.checked_add(body.text_offset)
                || symbol.size != body.byte_count
            {
                return Err(Diagnostic::error(
                    "entry body call changed its exact retained body placement",
                ));
            }
            validate_body_call_target(
                bridge.request().target.architecture,
                &output.final_text_bytes,
                region.section_offset,
                bridge.call_field_offset(),
                symbol.offset,
            )?;
            let end = region
                .section_offset
                .checked_add(region.byte_count)
                .ok_or_else(|| Diagnostic::error("final entry state byte span overflows"))?;
            let bytes = output
                .final_text_bytes
                .get(region.section_offset..end)
                .ok_or_else(|| Diagnostic::error("final entry state byte span is stale"))?;
            let call = Some(
                u32::try_from(bridge.call_field_offset())
                    .map_err(|_| Diagnostic::error("final entry body call exceeds u32"))?,
            );
            let replay = match artifact.target().architecture {
                target::Architecture::X86_64 => {
                    isa_x86_64::decode_x86_64_generated_entry(bytes, call)
                }
                target::Architecture::Aarch64 => {
                    isa_aarch64::decode_aarch64_generated_entry(bytes, call)
                }
            }
            .map_err(Diagnostic::error)?;
            if replay != *bridge.generated()
                || region.footprint.as_ref() != Some(replay.effects.footprint())
            {
                return Err(Diagnostic::error(
                    "final entry instructions or effects differ from target replay",
                ));
            }
        }
    }
    if let Some(shim) = shim
        && shim.surface == EntrySurface::Physical
    {
        let (_, offset) = shim.symbol_offset();
        let (expected, effects) = crate::entries::physical_entry::generated_state_effects(artifact)
            .ok_or_else(|| Diagnostic::error("physical entry lost its checked state evidence"))?;
        let region = output
            .executable_regions
            .regions
            .iter()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::CompilerFunction
                    && region.section_offset == offset
            })
            .ok_or_else(|| Diagnostic::error("physical entry state has no exact placed region"))?;
        let end = offset
            .checked_add(region.byte_count)
            .ok_or_else(|| Diagnostic::error("physical state span overflows"))?;
        let bytes = output
            .final_text_bytes
            .get(offset..end)
            .ok_or_else(|| Diagnostic::error("physical state span is stale"))?;
        let call = expected.instructions.iter().find_map(|row| {
            if let calling_conventions::GeneratedEntryControl::Call { field_byte_offset } =
                row.control
            {
                Some(field_byte_offset)
            } else {
                None
            }
        });
        let replay =
            isa_x86_64::decode_x86_64_generated_entry(bytes, call).map_err(Diagnostic::error)?;
        if replay != *expected
            || region.entry_effects.as_ref() != Some(effects)
            || region.footprint.as_ref() != Some(replay.effects.footprint())
        {
            return Err(Diagnostic::error(
                "physical entry instruction/state evidence fails exact target replay",
            ));
        }
    }
    Ok(())
}

/// The call form is independently decoded above; replay its relocation value
/// against the retained body, not merely the set of bits the writer may change.
/// Both ends are in .text, so the common section address cancels.
pub(crate) fn validate_body_call_target(
    architecture: target::Architecture,
    final_text: &[u8],
    entry_offset: usize,
    call_field_offset: usize,
    body_offset: usize,
) -> Result<(), Diagnostic> {
    let field = entry_offset
        .checked_add(call_field_offset)
        .ok_or_else(|| Diagnostic::error("entry body call field overflows"))?;
    let target = u64::try_from(body_offset)
        .map_err(|_| Diagnostic::error("entry body target exceeds u64"))?;
    let kind = match architecture {
        target::Architecture::X86_64 => object_file::RelocationKind::X86_64Relative32,
        target::Architecture::Aarch64 => object_file::RelocationKind::Aarch64Branch26,
    };
    image::validate_relative_relocation_target(kind, final_text, field, 0, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_body_target_cannot_reuse_equal_instruction_effect_rows() {
        for (architecture, original, changed, field) in [
            (
                target::Architecture::X86_64,
                vec![0xe8, 0xeb, 0xff, 0xff, 0xff, 0xc3],
                vec![0xe8, 0xef, 0xff, 0xff, 0xff, 0xc3],
                1,
            ),
            (
                target::Architecture::Aarch64,
                vec![0xfc, 0xff, 0xff, 0x97, 0xc0, 0x03, 0x5f, 0xd6],
                vec![0xfd, 0xff, 0xff, 0x97, 0xc0, 0x03, 0x5f, 0xd6],
                0,
            ),
        ] {
            let decode = |bytes: &[u8]| match architecture {
                target::Architecture::X86_64 => {
                    isa_x86_64::decode_x86_64_generated_entry(bytes, Some(field))
                }
                target::Architecture::Aarch64 => {
                    isa_aarch64::decode_aarch64_generated_entry(bytes, Some(field))
                }
            };
            assert_eq!(decode(&original).unwrap(), decode(&changed).unwrap());
            let mut text = match architecture {
                target::Architecture::X86_64 => vec![0xc3; 16],
                target::Architecture::Aarch64 => [0xc0, 0x03, 0x5f, 0xd6].repeat(4),
            };
            text.extend_from_slice(&original);
            validate_body_call_target(architecture, &text, 16, field as usize, 0).unwrap();
            text[16..].copy_from_slice(&changed);
            assert!(validate_body_call_target(architecture, &text, 16, field as usize, 0).is_err());
        }
    }
}
