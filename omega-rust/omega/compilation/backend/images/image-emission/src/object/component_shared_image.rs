//! Separately emitted callable image custody for ordinary ELF, Mach-O, and Windows PE loaders.
//!
//! This owns the original object/bridge beside its distinct shared container.
//! It cannot enter the process ExecutableImage or InstalledCode routes merely
//! because bytes were emitted. Runtime installation binds the component graph,
//! entry/resource contracts and actual loaded occurrences independently.

use diagnostics::Diagnostic;
use machine_emission::native_entry_bridge::NativeEntryBridge;

/// Distinct format containers retain their own original image/export inputs.
/// Neither variant is a process executable or an InstalledCode authority.
#[derive(Debug)]
pub enum ComponentSharedContainer {
    Elf(image_elf::ElfSharedComponentImage),
    MachO(image_macho::MachoSharedComponentImage),
    Pe(image_pe::PeSharedComponentImage),
}

impl ComponentSharedContainer {
    pub const fn image(&self) -> &image::FinalImage {
        match self {
            Self::Elf(image) => image.image(),
            Self::MachO(image) => image.image(),
            Self::Pe(image) => image.image(),
        }
    }
    pub fn final_text_bytes(&self) -> &[u8] {
        match self {
            Self::Elf(image) => image.final_text_bytes(),
            Self::MachO(image) => image.final_text_bytes(),
            Self::Pe(image) => image.final_text_bytes(),
        }
    }
    pub fn contents(&self) -> &[u8] {
        match self {
            Self::Elf(image) => image.contents(),
            Self::MachO(image) => image.contents(),
            Self::Pe(image) => image.contents(),
        }
    }
}

pub struct ComponentSharedImage {
    body: crate::ObjectArtifact,
    entry_bridge: Option<NativeEntryBridge>,
    image: ComponentSharedContainer,
    slot_commitment: [u8; 32],
}

impl ComponentSharedImage {
    pub const fn body(&self) -> &crate::ObjectArtifact {
        &self.body
    }
    pub const fn entry_bridge(&self) -> Option<&NativeEntryBridge> {
        self.entry_bridge.as_ref()
    }
    pub const fn image(&self) -> &ComponentSharedContainer {
        &self.image
    }
    pub const fn slot_commitment(&self) -> &[u8; 32] {
        &self.slot_commitment
    }

    /// Derive this retained body's demand with its actual entry transport.
    /// Consumers compare this physical magnitude, not a provider-written row
    /// or compact execution report, before admitting a component process graph.
    pub fn stack_demand(&self) -> Result<crate::StackDemand, Diagnostic> {
        let mut demand = crate::derive_stack_demand(&self.body, self.body.entry())
            .map_err(|error| Diagnostic::error(format!("component stack demand: {error:?}")))?;
        if let Some(bridge) = &self.entry_bridge {
            demand.stack_alignment = demand
                .stack_alignment
                .max(u32::from(bridge.request().inbound.call.stack_alignment))
                .max(u32::from(bridge.request().internal.stack_alignment));
            demand.ceiling_bytes = demand
                .ceiling_bytes
                .checked_add(u64::from(bridge.frame_bytes()))
                .and_then(|bytes| bytes.checked_add(u64::from(bridge.call_stack_bias())))
                .ok_or_else(|| Diagnostic::error("component entry stack demand overflows"))?;
        }
        Ok(demand)
    }
}

pub struct ComponentSharedImageEmissionError {
    pub function: crate::ComponentFunctionImage,
    pub diagnostic: Diagnostic,
}

pub fn emit_component_shared_image(
    function: crate::ComponentFunctionImage,
) -> Result<ComponentSharedImage, Box<ComponentSharedImageEmissionError>> {
    let crate::ComponentFunctionImage {
        body,
        entry_bridge,
        image,
        export_symbol,
        slot_commitment,
    } = function;
    let emitted = emit_shared_function_container(
        image,
        export_symbol,
        slot_commitment,
        crate::component_export_symbol(&slot_commitment).into_bytes(),
    );
    let emitted = emitted.and_then(|image| {
        let validation =
            validate_component_entry_effects(&image, export_symbol, entry_bridge.as_ref());
        match validation {
            Ok(()) => Ok(image),
            Err(diagnostic) => Err((image.image().clone(), diagnostic)),
        }
    });
    match emitted {
        Ok(image) => Ok(ComponentSharedImage {
            body,
            entry_bridge,
            image,
            slot_commitment,
        }),
        Err((image, diagnostic)) => Err(Box::new(ComponentSharedImageEmissionError {
            function: crate::ComponentFunctionImage {
                body,
                entry_bridge,
                image,
                export_symbol,
                slot_commitment,
            },
            diagnostic,
        })),
    }
}

/// Physical container dispatch shared by checked body exports and foreign ABI
/// adapters. The caller retains the semantic admission and original function.
pub(crate) fn emit_shared_function_container(
    image: image::FinalImage,
    export_symbol: image::FinalImageSymbolHandle,
    slot_commitment: [u8; 32],
    name: Vec<u8>,
) -> Result<ComponentSharedContainer, (image::FinalImage, Diagnostic)> {
    match image.target.object_format {
        target::ObjectFormat::Elf => image_elf::emit_elf_shared_component(
            image,
            vec![image_elf::ElfComponentExport {
                symbol: export_symbol,
                name,
                slot_commitment,
            }],
        )
        .map(ComponentSharedContainer::Elf)
        .map_err(|error| {
            let (image, _, diagnostic) = error.into_parts();
            (image, diagnostic)
        }),
        target::ObjectFormat::MachO => image_macho::emit_macho_shared_component(
            image,
            vec![image_macho::MachoComponentExport {
                symbol: export_symbol,
                name,
                slot_commitment,
            }],
        )
        .map(ComponentSharedContainer::MachO)
        .map_err(|error| {
            let (image, _, diagnostic) = error.into_parts();
            (image, diagnostic)
        }),
        target::ObjectFormat::Coff if image.target == target::NativeTarget::windows_x64() => {
            image_pe::emit_pe_shared_component(
                image,
                vec![image_pe::PeComponentExport {
                    symbol: export_symbol,
                    name,
                    slot_commitment,
                }],
            )
            .map(ComponentSharedContainer::Pe)
            .map_err(|error| {
                let (image, _, diagnostic) = error.into_parts();
                (image, diagnostic)
            })
        }
        _ => Err((
            image,
            Diagnostic::error(
                "shared component requires an admitted ELF, Mach-O, or Windows PE container",
            ),
        )),
    }
}

/// Replay only the selected entry rows through their target form owner after
/// the format owner has resolved its relocations. Body instruction custody
/// remains with the original checked fragment; this is not a whole-image decoder.
fn validate_component_entry_effects(
    container: &ComponentSharedContainer,
    export: image::FinalImageSymbolHandle,
    bridge: Option<&NativeEntryBridge>,
) -> Result<(), Diagnostic> {
    let image = container.image();
    let symbol = image.symbol_table.symbols.get(export);
    let region = image
        .executable_regions
        .iter()
        .find(|region| region.section_offset == symbol.offset && region.byte_count == symbol.size)
        .ok_or_else(|| Diagnostic::error("component entry has no exact state-evidence region"))?;
    let complete = region
        .entry_effects
        .as_ref()
        .ok_or_else(|| Diagnostic::error("component entry lost its complete state evidence"))?;
    if let Some(bridge) = bridge {
        if complete != bridge.complete_effects() {
            return Err(Diagnostic::error(
                "component entry state differs from its checked closure",
            ));
        }
        let end = symbol
            .offset
            .checked_add(symbol.size)
            .ok_or_else(|| Diagnostic::error("component entry byte span overflows"))?;
        let bytes = container
            .final_text_bytes()
            .get(symbol.offset..end)
            .ok_or_else(|| Diagnostic::error("component entry byte span is stale"))?;
        let call = Some(
            u32::try_from(bridge.call_field_offset())
                .map_err(|_| Diagnostic::error("component entry call exceeds u32"))?,
        );
        let replay = match image.target.architecture {
            target::Architecture::X86_64 => isa_x86_64::decode_x86_64_generated_entry(bytes, call),
            target::Architecture::Aarch64 => {
                isa_aarch64::decode_aarch64_generated_entry(bytes, call)
            }
        }
        .map_err(Diagnostic::error)?;
        if replay != *bridge.generated()
            || region.footprint.as_ref() != Some(replay.effects.footprint())
        {
            return Err(Diagnostic::error(
                "component entry instructions or effects fail final target replay",
            ));
        }
    }
    Ok(())
}
