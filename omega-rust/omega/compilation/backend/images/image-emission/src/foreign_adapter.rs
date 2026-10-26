//! A loadable fixed-frame ABI adapter for one exact native import.
//!
//! The native producer emits this alongside its ordinary program. A receiving
//! host loads these bytes without a compiler, source evaluator or per-call JIT.
//! The complete calling application and selected provider still need admission;
//! neither a container nor its export symbol supplies that authority. The
//! retained bridge and locator let artifact readers inspect physical custody.

use diagnostics::Diagnostic;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use machine_emission::native_foreign_bridge::NativeForeignBridge;
use object_file::{RelocationKind, SymbolKind};

#[derive(Debug)]
pub struct NativeForeignAdapterImage {
    bridge: NativeForeignBridge,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
    export: String,
    container: crate::ComponentSharedContainer,
}

impl NativeForeignAdapterImage {
    pub const fn bridge(&self) -> &NativeForeignBridge {
        &self.bridge
    }
    pub const fn locator(&self) -> &target::NormalizedForeignLocator {
        &self.locator
    }
    pub const fn application(&self) -> &[u8; 32] {
        &self.application
    }
    /// C loader spelling. Mach-O's extra linker underscore stays in the image.
    pub fn export(&self) -> &str {
        &self.export
    }
    pub const fn container(&self) -> &crate::ComponentSharedContainer {
        &self.container
    }
}

#[derive(Debug)]
pub struct NativeForeignAdapterEmissionError {
    pub bridge: NativeForeignBridge,
    pub locator: target::NormalizedForeignLocator,
    pub application: [u8; 32],
    pub diagnostic: Diagnostic,
}

/// Emit ordinary PIC/shared-library bytes, with one direct call relocated to
/// the complete normalized import. This does not install or execute native code.
pub fn emit_native_foreign_adapter(
    bridge: NativeForeignBridge,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
) -> Result<NativeForeignAdapterImage, Box<NativeForeignAdapterEmissionError>> {
    let emitted = (|| {
        if application == [0; 32] || locator.target().native_target() != bridge.request().target {
            return Err(Diagnostic::error(
                "foreign ABI adapter has no exact application or matching locator target",
            ));
        }
        let mut identity = optimization_core::digest_encoder::DomainDigestEncoder::new(
            b"omega.native-foreign-frame-export.v1",
        );
        identity.raw(&application);
        identity.raw(&locator.identity_digest().as_bytes());
        // Scalar normalization can change code without changing a same-sized
        // physical shape. Its exact emitted body also owns the loader name.
        identity.bytes(bridge.bytes());
        let commitment = identity.finish();
        use std::fmt::Write;
        let mut export = String::from("__omega_foreign_frame_");
        for byte in commitment {
            write!(&mut export, "{byte:02x}").expect("writing into String");
        }
        let symbol = if bridge.request().target.object_format == target::ObjectFormat::MachO {
            format!("_{export}")
        } else {
            export.clone()
        };
        let mut image = FinalImage::with_capacity(
            bridge.request().target,
            FinalImageMemory {
                text: bridge.bytes().to_vec(),
                ..FinalImageMemory::default()
            },
            arena::Handle::invalid(),
            2,
            1,
            1,
        );
        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: symbol.clone(),
            section: FinalImageSection::Text,
            offset: 0,
            size: bridge.bytes().len(),
            kind: SymbolKind::Function,
        });
        image.symbol_table.entry_symbol = entry;
        // A normalized import owns its symbol spelling, version or ordinal.
        // This diagnostic label cannot replace any part of those coordinates.
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "foreign-adapter-import".into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::Normalized(locator.clone().into()),
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Text,
                offset: bridge.call_field_offset(),
                byte_width: 4,
                symbol_handle: imported,
                addend: 0,
                kind: match bridge.request().target.architecture {
                    target::Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
                    target::Architecture::X86_64 => RelocationKind::X86_64Relative32,
                },
            });
        crate::component_shared_image::emit_shared_function_container(
            image,
            entry,
            commitment,
            symbol.into_bytes(),
        )
        .map(|container| (container, export))
        .map_err(|(_, diagnostic)| diagnostic)
    })();
    match emitted {
        Ok((container, export)) => Ok(NativeForeignAdapterImage {
            bridge,
            locator,
            application,
            export,
            container,
        }),
        Err(diagnostic) => Err(Box::new(NativeForeignAdapterEmissionError {
            bridge,
            locator,
            application,
            diagnostic,
        })),
    }
}
