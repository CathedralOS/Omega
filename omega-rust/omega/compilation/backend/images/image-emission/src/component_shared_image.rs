//! Separately emitted callable image custody for the ordinary ELF loader.
//!
//! This owns the original object/bridge beside its distinct shared container.
//! It cannot enter the process ExecutableImage or InstalledCode routes merely
//! because bytes were emitted. Runtime installation binds the component graph,
//! entry/resource contracts and actual loaded occurrences independently.

use diagnostics::Diagnostic;
use machine_emission::native_entry_bridge::NativeEntryBridge;

pub struct ComponentSharedImage {
    body: crate::ObjectArtifact,
    entry_bridge: Option<NativeEntryBridge>,
    image: image_elf::ElfSharedComponentImage,
    slot_commitment: [u8; 32],
}

impl ComponentSharedImage {
    pub const fn body(&self) -> &crate::ObjectArtifact {
        &self.body
    }
    pub const fn entry_bridge(&self) -> Option<&NativeEntryBridge> {
        self.entry_bridge.as_ref()
    }
    pub const fn image(&self) -> &image_elf::ElfSharedComponentImage {
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
    let exports = vec![image_elf::ElfComponentExport {
        symbol: export_symbol,
        name: crate::component_export_symbol(&slot_commitment).into_bytes(),
        slot_commitment,
    }];
    match image_elf::emit_elf_shared_component(image, exports) {
        Ok(image) => Ok(ComponentSharedImage {
            body,
            entry_bridge,
            image,
            slot_commitment,
        }),
        Err(error) => {
            let (image, _, diagnostic) = error.into_parts();
            Err(Box::new(ComponentSharedImageEmissionError {
                function: crate::ComponentFunctionImage {
                    body,
                    entry_bridge,
                    image,
                    export_symbol,
                    slot_commitment,
                },
                diagnostic,
            }))
        }
    }
}
