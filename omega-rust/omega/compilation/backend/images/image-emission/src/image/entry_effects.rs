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
