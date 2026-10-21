//! Separately emitted callable image custody for ordinary ELF and Mach-O loaders.
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
}

impl ComponentSharedContainer {
    pub const fn image(&self) -> &image::FinalImage {
        match self {
            Self::Elf(image) => image.image(),
            Self::MachO(image) => image.image(),
        }
    }
    pub fn contents(&self) -> &[u8] {
        match self {
            Self::Elf(image) => image.contents(),
            Self::MachO(image) => image.contents(),
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
        _ => Err((
            image,
            Diagnostic::error("shared component requires an admitted ELF or Mach-O container"),
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
