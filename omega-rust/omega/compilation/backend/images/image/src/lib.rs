/*
We build the format-neutral final image (`FinalImage`) from an `ObjectPlan` and
its `RelocationPlan`, apply architecture-specific relocations in place once a
container backend supplies its chosen virtual section layout
(`FinalImageLayout`), and emit the strong SHA-256 and compact FNV-1a evidence
digests that downstream verifiers replay. No container headers are emitted
here: `image-elf`, `image-pe`, and `image-macho` each wrap `FinalImage` in
their own wire format while sharing our symbol table, relocation arithmetic,
and placed region inventories.

Our symbol handles (`FinalImageSymbolHandle`) deliberately reuse the raw
`(arena_index, generation)` coordinates of their source `ObjectSymbolHandle`
without building a secondary hash map. That zero-remap handoff holds because
`copy_object_symbols` populates a fresh symbol arena in a single pass over the
object's iteration order, and `copy_object_relocations` downgrades any handle
that fails `symbols.is_valid` to `Handle::invalid()`. Text and data relocation
envelopes are intentionally asymmetric: `.text` validates only the
compiler-encoded prefix because PE and Mach-O append import thunks after
compiler code, whereas `.data` requires every relocation to be an in-bounds,
non-overlapping, eight-byte-aligned `Absolute64` slot.
*/

/*
Every evidence carrier pairs a strong 32-byte SHA-256 digest over a versioned
domain string with a compact 64-bit FNV-1a reporting fingerprint. Because compact
fingerprints can collide, we enforce a strict `*_report_fingerprint` naming
discipline so compact coordinates never stand in for strong digests during
artifact admission or equivalence checks. When an import symbol carries both a
normalized foreign locator and a legacy `import_library` string, the normalized
locator wins unconditionally so object-local symbol spellings never override
target-validated import identity.
*/

/*
`image-elf`, `image-pe`, `image-macho`, and `image-emission` consume
`FinalImage`, `apply_relocations`, `place_executable_regions`, and
`place_data_regions` to produce and verify executable and shared-component
containers. Notice that `CompilerEntryRegionBindingEvidence` and
`CompilerEntryFootprintBindingEvidence` are destructured through field access
in `native-artifact` without naming their types directly, so both carriers
remain load-bearing across crate boundaries.
*/

macro_rules! image_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

#[macro_export]
macro_rules! rejection_carrier {
    ($name:ident; $($field:ident: $ty:ty),+ $(,)?) => {
        impl $name {
            pub const fn diagnostic(&self) -> &diagnostics::Diagnostic {
                &self.diagnostic
            }

            pub fn into_parts(self) -> ($($ty,)+ diagnostics::Diagnostic) {
                ($(self.$field,)+ self.diagnostic)
            }
        }

        diagnostics::diagnostic_display!($name);

        impl std::error::Error for $name {}
    };
}

mod builder;
pub mod codec_bytes {
    pub fn write_u16(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend(value.to_le_bytes());
    }

    pub fn write_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    pub fn write_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }

    #[cfg(test)]
    pub(crate) use optimization_core::codec_bytes::patch_encoded;
}

mod final_image;
pub(crate) use optimization_core::digest_encoder;
mod output;
mod patch_bytes {
    use std::ops::Range;

    use diagnostics::Diagnostic;

    pub(crate) fn read_u32(
        bytes: &[u8],
        offset: usize,
        relocation_name: &str,
    ) -> Result<u32, Diagnostic> {
        let slice = &bytes[relocation_range(bytes.len(), offset, 4, relocation_name)?];
        Ok(u32::from_le_bytes(
            slice.try_into().expect("u32 relocation slice has length 4"),
        ))
    }

    pub(crate) fn write_u32(
        bytes: &mut [u8],
        offset: usize,
        value: u32,
        relocation_name: &str,
    ) -> Result<(), Diagnostic> {
        write_bytes(bytes, offset, &value.to_le_bytes(), relocation_name)
    }

    pub(crate) fn write_u64(
        bytes: &mut [u8],
        offset: usize,
        value: u64,
        relocation_name: &str,
    ) -> Result<(), Diagnostic> {
        write_bytes(bytes, offset, &value.to_le_bytes(), relocation_name)
    }

    pub(crate) fn write_i32(
        bytes: &mut [u8],
        offset: usize,
        value: i32,
        relocation_name: &str,
    ) -> Result<(), Diagnostic> {
        write_bytes(bytes, offset, &value.to_le_bytes(), relocation_name)
    }

    fn write_bytes(
        bytes: &mut [u8],
        offset: usize,
        value: &[u8],
        relocation_name: &str,
    ) -> Result<(), Diagnostic> {
        let range = relocation_range(bytes.len(), offset, value.len(), relocation_name)?;
        bytes[range].copy_from_slice(value);
        Ok(())
    }

    fn relocation_range(
        bytes_len: usize,
        offset: usize,
        width: usize,
        relocation_name: &str,
    ) -> Result<Range<usize>, Diagnostic> {
        let end = offset.checked_add(width).ok_or_else(|| {
            Diagnostic::error(format!("{relocation_name} relocation offset overflow"))
        })?;
        if end > bytes_len {
            return Err(Diagnostic::error(format!(
                "{relocation_name} relocation offset {offset} is outside text section"
            )));
        }
        Ok(offset..end)
    }

    #[cfg(test)]
    mod tests {
        use super::write_u64;

        #[test]
        fn reports_out_of_bounds_relocation_patches() {
            let mut bytes = [0u8; 4];
            let diagnostic =
                write_u64(&mut bytes, 1, 0x1234, "test").expect_err("patch should exceed text");

            assert!(
                diagnostic
                    .message
                    .contains("test relocation offset 1 is outside text section")
            );
        }
    }
}

mod relocation_envelope;
mod relocations;
mod symbols {
    use crate::final_image::{
        FinalImage, FinalImageLayout, FinalImageSection, FinalImageSymbol, FinalImageSymbolHandle,
    };
    use arena::Handle;
    use object_file::{ObjectSymbolHandle, SectionKind, SymbolSection};

    pub(crate) fn final_image_symbol_handle(symbol: ObjectSymbolHandle) -> FinalImageSymbolHandle {
        if symbol.is_valid() {
            Handle::from_parts(symbol.arena_index(), symbol.generation())
        } else {
            FinalImageSymbolHandle::invalid()
        }
    }

    pub fn final_image_symbol_address(
        image: &FinalImage,
        symbol: FinalImageSymbolHandle,
        layout: &FinalImageLayout,
    ) -> Option<u64> {
        let symbol = image.symbol_table.symbol(symbol)?;
        let section_address = match symbol.section {
            FinalImageSection::Text => layout.text_address,
            FinalImageSection::Data => layout.data_address,
            FinalImageSection::Bss => layout.bss_address,
            FinalImageSection::None => return None,
        };

        Some(section_address + symbol.offset as u64)
    }

    pub fn final_image_imports_symbol(image: &FinalImage, symbol: FinalImageSymbolHandle) -> bool {
        image
            .symbol_table
            .imports
            .iter()
            .any(|(_, import)| import.symbol_handle == symbol)
    }

    pub fn final_image_symbol_name(image: &FinalImage, symbol: FinalImageSymbolHandle) -> &str {
        image
            .symbol_table
            .symbol(symbol)
            .map_or("", |symbol| symbol.name.as_str())
    }

    pub fn final_image_entry_symbol<'a>(
        image: &'a FinalImage,
        format: &'static str,
    ) -> Result<&'a FinalImageSymbol, diagnostics::Diagnostic> {
        let entry_symbol = image
            .symbol_table
            .symbol(image.symbol_table.entry_symbol)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "{format} entry symbol `{}` is missing from the final image",
                    final_image_symbol_name(image, image.symbol_table.entry_symbol)
                ))
            })?;

        if entry_symbol.section != FinalImageSection::Text {
            return Err(diagnostics::Diagnostic::error(format!(
                "{format} entry symbol `{}` is not in the text section",
                final_image_symbol_name(image, image.symbol_table.entry_symbol)
            )));
        }

        Ok(entry_symbol)
    }

    pub(crate) fn final_image_section(section: SymbolSection) -> FinalImageSection {
        match section {
            SymbolSection::None => FinalImageSection::None,
            SymbolSection::Section(SectionKind::Text) => FinalImageSection::Text,
            SymbolSection::Section(SectionKind::Data) => FinalImageSection::Data,
            SymbolSection::Section(SectionKind::Bss) => FinalImageSection::Bss,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{FinalImageImportPlan, FinalImageInput, build_final_image};
    use object_file::{
        NormalizedImportPlan, ObjectPlan, RelocationPlan, SymbolKind, SymbolPlan, SymbolSection,
    };
    use target::{ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator};

    #[test]
    fn final_image_keeps_normalized_import_atomic_and_ignores_symbol_spelling() {
        let target = NativeTarget::windows_x64();
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByOrdinal {
                library: b"raw\xff.dll".to_vec(),
                ordinal: 23,
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid PE locator");
        let mut object = ObjectPlan::with_capacity(target, 0, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: "diagnostic-only".into(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
            import_library: "must-not-win.dll".into(),
        });
        object.layout.normalized_imports.push(NormalizedImportPlan {
            symbol,
            locator: locator.clone().into(),
        });
        let relocations = RelocationPlan::with_target(target);

        let image = build_final_image(FinalImageInput {
            target,
            object: &object,
            relocations: &relocations,
            text_bytes: &[],
            data_bytes: &[],
        });
        let import = image.symbol_table.imports.iter().next().unwrap().1;
        assert_eq!(
            &import.import,
            &FinalImageImportPlan::Normalized((locator).into()),
            "normalized coordinates must win atomically over legacy symbol fields"
        );
    }

    #[test]
    fn object_kind_text_symbols_place_as_read_only_data_beside_code() {
        use crate::{FinalExecutableRegionOrigin, FinalImageLayout, place_executable_regions};
        use object_file::SectionKind;
        let target = NativeTarget::macos_arm64();
        let mut object = ObjectPlan::with_capacity(target, 1, 2);
        for (name, offset, size, kind) in [
            ("code", 0, 8, SymbolKind::Function),
            ("pool", 8, 8, SymbolKind::Object),
        ] {
            object.layout.symbols.insert(SymbolPlan {
                name: name.into(),
                section: SymbolSection::Section(SectionKind::Text),
                offset,
                size,
                kind,
                import_library: String::new(),
            });
        }
        let relocations = RelocationPlan::with_target(target);
        let text = [
            0x1f, 0x20, 0x03, 0xd5, 0x1f, 0x20, 0x03, 0xd5, b'D', b'o', b'o', b'r', 0, 0, 0, 0,
        ];
        let image = build_final_image(FinalImageInput {
            target,
            object: &object,
            relocations: &relocations,
            text_bytes: &text,
            data_bytes: &[],
        });
        let origins = image
            .executable_regions
            .iter()
            .map(|region| (region.symbol.as_str(), region.origin))
            .collect::<Vec<_>>();
        assert_eq!(
            origins,
            [
                ("code", FinalExecutableRegionOrigin::CompilerFunction),
                ("pool", FinalExecutableRegionOrigin::CompilerReadOnlyData),
            ]
        );
        let inventory = place_executable_regions(
            &image,
            FinalImageLayout {
                text_address: 0x1_0000_4000,
                data_address: 0,
                bss_address: 0,
            },
        )
        .unwrap();
        assert!(inventory.unclassified_gaps.is_empty());
    }

    #[test]
    fn executable_extent_uses_resolved_bytes_and_preserves_placement_refusals() {
        use crate::{
            FinalExecutableRegion, FinalExecutableRegionOrigin, place_executable_extent,
            validate_placed_executable_region_inventory,
        };
        let text = [11, 23, 47, 89, 131, 173];
        let region = FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 2,
            byte_count: 2,
            symbol: "resolved".to_owned(),
            footprint: None,
            entry_effects: None,
        };
        let inventory = place_executable_extent(&text, vec![region.clone()], 0x4000).unwrap();
        assert_eq!(inventory.text_byte_count, 6);
        assert_eq!(inventory.regions[0].address, 0x4002);
        assert_eq!(inventory.unclassified_gaps.len(), 2);
        assert_eq!(inventory.unclassified_gaps[0].byte_count, 2);
        assert_eq!(inventory.unclassified_gaps[1].section_offset, 4);
        validate_placed_executable_region_inventory(&inventory, &text).unwrap();
        for index in [0, 2, 5] {
            let mut changed = text;
            changed[index] ^= 1;
            assert!(validate_placed_executable_region_inventory(&inventory, &changed).is_err());
        }
        let overlapping = FinalExecutableRegion {
            section_offset: 3,
            ..region.clone()
        };
        assert!(
            place_executable_extent(&text, vec![region.clone(), overlapping], 0x4000)
                .unwrap_err()
                .to_string()
                .contains("overlaps")
        );
        let outside = FinalExecutableRegion {
            section_offset: 5,
            ..region.clone()
        };
        assert!(
            place_executable_extent(&text, vec![outside], 0x4000)
                .unwrap_err()
                .to_string()
                .contains("exceeds .text size")
        );
        assert!(
            place_executable_extent(&text, vec![region], u64::MAX)
                .unwrap_err()
                .to_string()
                .contains("address overflows")
        );
        let empty = place_executable_extent(&[], Vec::new(), 0).unwrap();
        validate_placed_executable_region_inventory(&empty, &[]).unwrap();
    }
}

pub use builder::{FinalImageInput, build_final_image};
pub use final_image::{
    FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
    FinalExecutableTextDigest, FinalImage, FinalImageImport, FinalImageImportPlan,
    FinalImageLayout, FinalImageMemory, FinalImageRelocation, FinalImageRelocationTable,
    FinalImageSection, FinalImageSymbol, FinalImageSymbolDigest, FinalImageSymbolHandle,
    FinalInitializedDataDigest, PlacedDataGapBytesDigest, PlacedDataRegionBytesDigest,
    PlacedDataRegionInventory, PlacedDataRegionInventoryDigest, PlacedExecutableGapBytesDigest,
    PlacedExecutableRegion, PlacedExecutableRegionBytesDigest, PlacedExecutableRegionInventory,
    PlacedExecutableRegionInventoryDigest, StateFootprintEvidenceDigest, final_image_symbol_digest,
    place_data_extent, place_data_regions, place_executable_extent, place_executable_regions,
    validate_placed_data_region_inventory, validate_placed_executable_region_inventory,
};
pub use output::{
    CompilerEntryFootprintBindingDigest, CompilerEntryRegionBindingDigest,
    CompilerFunctionValidationDigest, CompilerFunctionValidationEvidence,
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest,
    CompilerTextValidationEvidence, EmittedImageOutput, EncodedCompilerTextDigest,
    ExecutableImageOutput, FinalCompilerTextDigest, ImageOutputKind,
    emitted_direct_executable_output,
};
pub use relocation_envelope::validate_final_text_relocation_envelope;
pub use relocations::{apply_relocations, validate_relative_relocation_target};
pub use symbols::{
    final_image_entry_symbol, final_image_imports_symbol, final_image_symbol_address,
    final_image_symbol_name,
};

pub fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

pub fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
