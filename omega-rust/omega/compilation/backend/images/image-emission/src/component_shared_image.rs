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
                .max(u32::from(bridge.request().inbound.stack_alignment))
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
    let name = crate::component_export_symbol(&slot_commitment).into_bytes();
    let emitted = match image.target.object_format {
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
    };
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
