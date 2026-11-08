/*
We emit position-independent (PIE), ad-hoc code-signed, dyld-linked Mach-O
`MH_EXECUTE` executables (`emit_macho_executable`, `emit_macho_executable_signed`)
and `MH_DYLIB` shared components (`emit_macho_shared_component`) for macOS
AArch64 and x86-64. Executables map a 4 GiB unmapped `__PAGEZERO` segment below
`IMAGE_BASE = 0x1_0000_0000`, align loader segments to the ISA page size
(16 KiB on AArch64, 4 KiB on x86-64), bind all imported symbols eagerly at load
time through `__DATA,__got` pointer slots, and embed a SHA-256 `CodeDirectory`
superblob at the end of `__LINKEDIT`.

Emission proceeds in a strict transaction order: we preflight the referenced
dylib roster (capped at 15 dylibs by the 4-bit dyld bind ordinal immediate,
with `libSystem.B.dylib` anchored at ordinal 1) before mutating `FinalImage`,
install import thunks and pointer slots, encode the dyld bind and PIE rebase
opcode streams, plan all container offsets in a single pass, apply relocations
and re-verify rebased pointers and thunk opcodes, serialize the Mach-O header,
load commands, segments, and `__LINKEDIT` payloads, sign the completed prefix,
and replay the container mapping and loader fixups against the finished bytes.
*/

/*
We emit only referenced imports into the dylib roster, bind stream, and GOT so
programs do not pull in unused macOS frameworks at load time. Because
`LC_CODE_SIGNATURE` and `__LINKEDIT` sizes are written before the file bytes
exist to be hashed, `CodeSignatureLayout` derives both the planned signature
extent and the final superblob layout from a single calculation over the code
limit and signing identifier, preventing any drift between the load commands
and the trailing signature blob.
*/

/*
`image` supplies `FinalImage`, `apply_relocations`, and region placement;
`file_layout.rs` owns the container offset plan; `dyld_linking.rs` owns import
thunks, rebase/bind streams, load commands, and loader replay verifiers;
`code_signature.rs` owns ad-hoc signing; and `shared_component.rs` owns
`MH_DYLIB` export tries and component emission.
*/

pub(crate) use optimization_core::codec_bytes;

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalImage, PlacedDataRegionInventory, place_data_regions,
    place_executable_regions,
};

mod shared_component;
pub use shared_component::{
    MachoComponentExport, MachoSharedComponentEmissionError, MachoSharedComponentImage,
    emit_macho_shared_component,
};
mod code_signature;
pub use code_signature::code_signature_identifier;
mod dyld_linking;
mod entry_symbol {
    use diagnostics::Diagnostic;
    use image::FinalImage;

    pub(crate) fn macho_entry_text_offset(image: &FinalImage) -> Result<usize, Diagnostic> {
        Ok(image::final_image_entry_symbol(image, "Mach-O")?.offset)
    }

    #[cfg(test)]
    mod tests {
        use super::macho_entry_text_offset;
        use image::{FinalImage, FinalImageSection, FinalImageSymbol};

        #[test]
        fn resolves_macho_entry_offset_from_final_image_entry_symbol() {
            let mut image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    text: vec![0; 16],
                    ..Default::default()
                },
            );
            let entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "_start".into(),
                section: FinalImageSection::Text,
                offset: 8,
                size: 4,
                ..FinalImageSymbol::default()
            });
            image.symbol_table.entry_symbol = entry_symbol;

            assert_eq!(macho_entry_text_offset(&image).unwrap(), 8);
        }
    }
}

mod file_layout;
mod isa {
    use calling_conventions::StateFootprintEvidence;
    use diagnostics::Diagnostic;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum MachoIsa {
        Aarch64,
        X86_64,
    }

    pub(crate) fn macho_isa_for(target: target::NativeTarget) -> Result<MachoIsa, Diagnostic> {
        MachoIsa::from_format(target.object_format, target.architecture).ok_or_else(|| {
            Diagnostic::error(format!(
                "Mach-O emission and validation require a macOS ARM64 or x86-64 target, not {:?}",
                target,
            ))
        })
    }

    impl MachoIsa {
        pub(crate) fn from_format(
            object_format: target::ObjectFormat,
            architecture: target::Architecture,
        ) -> Option<Self> {
            match (object_format, architecture) {
                (target::ObjectFormat::MachO, target::Architecture::Aarch64) => Some(Self::Aarch64),
                (target::ObjectFormat::MachO, target::Architecture::X86_64) => Some(Self::X86_64),
                _ => None,
            }
        }

        pub(crate) fn native_target(self) -> target::NativeTarget {
            match self {
                Self::Aarch64 => target::NativeTarget::macos_arm64(),
                Self::X86_64 => target::NativeTarget::macos_x64(),
            }
        }

        pub(crate) fn target_profile(self) -> target::TargetProfile {
            match self {
                Self::Aarch64 => target::TargetProfile::MacosArm64,
                Self::X86_64 => target::TargetProfile::MacosX64,
            }
        }

        pub(crate) fn cpu_type(self) -> u32 {
            match self {
                Self::Aarch64 => 0x0100_000c,
                Self::X86_64 => 0x0100_0007,
            }
        }

        pub(crate) fn cpu_subtype(self) -> u32 {
            match self {
                Self::Aarch64 => 0,
                Self::X86_64 => 0x8000_0003,
            }
        }

        pub(crate) fn page_size(self) -> u64 {
            match self {
                Self::Aarch64 => 0x4000,
                Self::X86_64 => 0x1000,
            }
        }

        pub(crate) fn code_signature_page_power(self) -> u8 {
            match self {
                Self::Aarch64 => 14,
                Self::X86_64 => 12,
            }
        }

        pub(crate) fn import_thunk_size(self) -> usize {
            match self {
                Self::Aarch64 => 12,
                Self::X86_64 => isa_x86_64::X86_64_IMPORT_THUNK_BYTE_COUNT,
            }
        }

        pub(crate) fn entry_alignment(self) -> u64 {
            match self {
                Self::Aarch64 => 4,
                Self::X86_64 => 1,
            }
        }

        pub(crate) fn format_name(self) -> &'static str {
            match self {
                Self::Aarch64 => "mach-o-arm64-executable",
                Self::X86_64 => "mach-o-x86_64-executable",
            }
        }

        pub(crate) fn thunk_footprint(self) -> StateFootprintEvidence {
            match self {
                Self::Aarch64 => StateFootprintEvidence::new(
                    calling_conventions::RegisterSet::new([
                        calling_conventions::MachineRegister::Aarch64X(16),
                    ]),
                    calling_conventions::MachineStateSet::new([
                        calling_conventions::MachineState::InstructionPointer,
                    ]),
                ),
                Self::X86_64 => isa_x86_64::x86_64_import_thunk_footprint(),
            }
        }
    }
}

#[cfg(test)]
mod test_support {
    use arena::Handle;
    use image::{FinalImage, FinalImageMemory, FinalImageRelocation};

    pub(crate) fn image(target: target::NativeTarget, memory: FinalImageMemory) -> FinalImage {
        FinalImage::with_capacity(target, memory, Handle::invalid(), 0, 0, 0)
    }

    pub(crate) fn relocation_mut(
        image: &mut FinalImage,
        handle: Handle<FinalImageRelocation>,
    ) -> &mut FinalImageRelocation {
        image.relocation_table.relocations.get_mut(handle)
    }
}

#[cfg(test)]
mod tests {
    use image::{
        FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
    };
    use object_file::{RelocationKind, SymbolKind};

    fn word(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn wide(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
    }

    fn segment<'image>(bytes: &'image [u8], name: &[u8; 16]) -> &'image [u8] {
        let mut cursor = 32;
        let mut matching = None;
        for _ in 0..word(bytes, 16) {
            let end = cursor + word(bytes, cursor + 4) as usize;
            if word(bytes, cursor) == 0x19 && bytes.get(cursor + 8..cursor + 24) == Some(name) {
                assert!(matching.replace(&bytes[cursor..end]).is_none());
            }
            cursor = end;
        }
        assert_eq!(cursor, 32 + word(bytes, 20) as usize);
        matching.expect("one exact segment")
    }

    fn storage_image(data_size: usize, bss_size: usize, bss_alignment: usize) -> FinalImage {
        storage_image_for(
            target::NativeTarget::macos_arm64(),
            0xd65f_03c0u32.to_le_bytes().to_vec(), // ret
            data_size,
            bss_size,
            bss_alignment,
        )
    }

    fn storage_image_for(
        target: target::NativeTarget,
        text: Vec<u8>,
        data_size: usize,
        bss_size: usize,
        bss_alignment: usize,
    ) -> FinalImage {
        let text_size = text.len();
        let mut image = crate::test_support::image(
            target,
            FinalImageMemory {
                text,
                data: vec![0; data_size],
                bss_size,
                bss_alignment,
            },
        );
        image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "entry".into(),
            section: FinalImageSection::Text,
            size: text_size,
            kind: SymbolKind::Function,
            ..Default::default()
        });
        if data_size >= 8 && bss_size > 0 {
            let storage = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "storage".into(),
                section: FinalImageSection::Bss,
                size: bss_size,
                ..Default::default()
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Data,
                    byte_width: 8,
                    symbol_handle: storage,
                    kind: RelocationKind::Absolute64,
                    ..Default::default()
                });
        }
        image
    }

    #[test]
    fn emitted_bss_matches_relocated_storage_and_writable_segment() {
        for (data_size, bss_size, alignment) in [
            (13, 24, 16),
            (13, 0x4000, 0x4000),
            (16, 24, 16),
            (0, 24, 16),
            (0, 0x8000, 0x10000),
        ] {
            let output =
                super::emit_macho_executable(storage_image(data_size, bss_size, alignment))
                    .expect("emit storage image");
            let data = segment(&output.bytes, b"__DATA\0\0\0\0\0\0\0\0\0\0");
            let (sections, remainder) = data[72..].as_chunks::<80>();
            assert!(remainder.is_empty());
            assert_eq!(sections.len(), word(data, 64) as usize);
            let bss = sections
                .iter()
                .find(|section| &section[..16] == b"__bss\0\0\0\0\0\0\0\0\0\0\0")
                .expect("BSS section");
            let bss_address = wide(bss, 32);
            assert_eq!(
                bss_address, output.final_image_layout.bss_address,
                "loader and relocation disagree for data={data_size}, alignment={alignment}"
            );
            assert_eq!(bss_address % alignment as u64, 0);
            assert_eq!(wide(bss, 40), bss_size as u64);
            assert_eq!(word(bss, 48), 0, "zero-fill has no file payload");
            assert_eq!(word(bss, 52), alignment.trailing_zeros());
            assert_eq!(word(bss, 64), 1, "S_ZEROFILL");
            assert_eq!(word(data, 56), 3, "maximum read/write, non-executable");
            assert_eq!(word(data, 60), 3, "initial read/write, non-executable");
            assert_eq!(wide(data, 48), data_size as u64);
            assert!(bss_address >= wide(data, 24) + data_size as u64);
            assert!(bss_address + bss_size as u64 <= wide(data, 24) + wide(data, 32));
            let linkedit = segment(&output.bytes, b"__LINKEDIT\0\0\0\0\0\0");
            assert!(wide(linkedit, 24) >= bss_address + bss_size as u64);
            if data_size >= 8 {
                assert_eq!(wide(&output.final_data_bytes, 0), bss_address);
                let file_offset = wide(data, 40) as usize;
                assert_eq!(wide(&output.bytes, file_offset), bss_address);
            }
        }
    }

    fn validate_mapping(
        output: &image::ExecutableImageOutput,
    ) -> Result<(), diagnostics::Diagnostic> {
        super::validate_macho_loader_mapping(
            &output.bytes,
            output.final_image_layout,
            &output.final_text_bytes,
            &output.final_data_bytes,
            output.bss_bytes,
            target::NativeTarget::macos_arm64(),
        )
    }

    fn x86_64_storage_image(data_size: usize, bss_size: usize, bss_alignment: usize) -> FinalImage {
        storage_image_for(
            target::NativeTarget::macos_x64(),
            vec![0xc3], // ret
            data_size,
            bss_size,
            bss_alignment,
        )
    }

    fn validate_x86_64_mapping(
        output: &image::ExecutableImageOutput,
    ) -> Result<(), diagnostics::Diagnostic> {
        super::validate_macho_loader_mapping(
            &output.bytes,
            output.final_image_layout,
            &output.final_text_bytes,
            &output.final_data_bytes,
            output.bss_bytes,
            target::NativeTarget::macos_x64(),
        )
    }

    #[test]
    fn x86_64_emission_writes_intel_cpu_fields_and_four_kib_pages() {
        let output =
            super::emit_macho_executable(x86_64_storage_image(13, 24, 16)).expect("emit x86-64");
        assert_eq!(output.format, "mach-o-x86_64-executable");
        assert_eq!(word(&output.bytes, 0), 0xfeed_facf);
        assert_eq!(word(&output.bytes, 4), 0x0100_0007, "CPU_TYPE_X86_64");
        assert_eq!(
            word(&output.bytes, 8),
            0x8000_0003,
            "CPU_SUBTYPE_X86_64_ALL|LIB64"
        );
        let data = segment(&output.bytes, b"__DATA\0\0\0\0\0\0\0\0\0\0");
        assert_eq!(wide(data, 40), 0x1000);
        assert_eq!(output.final_image_layout.data_address, 0x1_0000_1000);
        validate_x86_64_mapping(&output).expect("x86-64 loader mapping");
    }

    #[test]
    fn x86_64_loader_mapping_accepts_storage_variations() {
        for (data_size, bss_size, alignment) in [
            (0, 0, 1),
            (13, 0, 8),
            (13, 0, 0x10000),
            (0, 24, 16),
            (13, 24, 16),
            (13, 0x4000, 0x4000),
            (0, 0x8000, 0x10000),
        ] {
            let output =
                super::emit_macho_executable(x86_64_storage_image(data_size, bss_size, alignment))
                    .expect("emit x86-64 storage variation");
            validate_x86_64_mapping(&output).expect("exact x86-64 loader mapping");
        }
    }

    #[test]
    fn x86_64_loader_mapping_accepts_eager_import_storage() {
        let mut image = storage_image_for(
            target::NativeTarget::macos_x64(),
            vec![0xc3, 0, 0, 0],
            13,
            24,
            16,
        );
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_write".into(),
            kind: SymbolKind::Import,
            ..Default::default()
        });
        image.symbol_table.imports.insert(image::FinalImageImport {
            symbol_handle: imported,
            import: image::FinalImageImportPlan::StringBackedBootstrap {
                library: String::new(),
            },
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Text,
                byte_width: 4,
                symbol_handle: imported,
                kind: RelocationKind::X86_64Relative32,
                ..Default::default()
            });
        let output = super::emit_macho_executable(image).expect("emit x86-64 eager import");
        validate_x86_64_mapping(&output).expect("x86-64 eager import loader mapping");
        let thunk = output
            .executable_regions
            .regions
            .iter()
            .find(|region| region.origin == image::FinalExecutableRegionOrigin::ImportThunk)
            .expect("x86-64 import thunk region");
        assert_eq!(thunk.byte_count, 6);
        let thunk_bytes = &output.final_text_bytes[thunk.section_offset..thunk.section_offset + 6];
        assert_eq!(&thunk_bytes[..2], &[0xff, 0x25]);
        super::validate_macho_import_binding_pairing(
            &output.final_text_bytes,
            &output.executable_regions,
            &output.data_regions,
            target::NativeTarget::macos_x64(),
        )
        .expect("x86-64 thunk↔slot pairing");
    }

    #[test]
    fn macho_emission_refuses_a_target_off_the_two_supported_diagonals() {
        let elf = storage_image_for(target::NativeTarget::linux_x64(), vec![0xc3], 0, 0, 1);
        assert!(super::emit_macho_executable(elf).is_err());
    }

    fn segment_offset(bytes: &[u8], name: &[u8; 16]) -> usize {
        segment(bytes, name).as_ptr() as usize - bytes.as_ptr() as usize
    }

    #[test]
    fn loader_mapping_accepts_storage_variations() {
        for (data_size, bss_size, alignment) in [
            (0, 0, 1),
            (13, 0, 8),
            (13, 0, 0x10000),
            (0, 24, 16),
            (13, 24, 16),
            (13, 0x4000, 0x4000),
            (0, 0x8000, 0x10000),
        ] {
            let output =
                super::emit_macho_executable(storage_image(data_size, bss_size, alignment))
                    .expect("emit storage variation");
            validate_mapping(&output).expect("exact loader mapping");
        }
    }

    #[test]
    fn loader_mapping_accepts_eager_import_storage() {
        for name in ["_write", "_objc_msgSend"] {
            let mut image = storage_image(13, 24, 16);
            crate::codec_bytes::patch_encoded(
                &mut image.memory.text,
                0,
                0x9400_0000u32.to_le_bytes(),
            );
            let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: name.into(),
                kind: SymbolKind::Import,
                ..Default::default()
            });
            image.symbol_table.imports.insert(image::FinalImageImport {
                symbol_handle: imported,
                import: image::FinalImageImportPlan::StringBackedBootstrap {
                    library: String::new(),
                },
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    byte_width: 4,
                    symbol_handle: imported,
                    kind: RelocationKind::Aarch64Branch26,
                    ..Default::default()
                });
            let output = super::emit_macho_executable(image).expect("emit eager import");
            validate_mapping(&output).expect("eager import loader mapping");
        }
    }

    #[test]
    fn loader_mapping_rejects_segment_and_zero_fill_corruption() {
        let original =
            super::emit_macho_executable(storage_image(13, 24, 16)).expect("emit storage image");
        let data_offset = segment_offset(&original.bytes, b"__DATA\0\0\0\0\0\0\0\0\0\0");
        let text_offset = segment_offset(&original.bytes, b"__TEXT\0\0\0\0\0\0\0\0\0\0");
        let linkedit_offset = segment_offset(&original.bytes, b"__LINKEDIT\0\0\0\0\0\0");
        for mutation in 0..10 {
            let mut output = original.clone();
            match mutation {
                0 => output.bytes[data_offset + 68..data_offset + 72]
                    .copy_from_slice(&1u32.to_le_bytes()),
                1 => output.bytes[data_offset + 40..data_offset + 48]
                    .copy_from_slice(&0u64.to_le_bytes()),
                2 => {
                    output.bytes[40..56].copy_from_slice(b"__ALIAS\0\0\0\0\0\0\0\0\0");
                    output.bytes[56..64]
                        .copy_from_slice(&output.final_image_layout.data_address.to_le_bytes());
                }
                3 => output.bytes[data_offset + 60..data_offset + 64]
                    .copy_from_slice(&7u32.to_le_bytes()),
                4 => output.bytes[text_offset + 24..text_offset + 32]
                    .copy_from_slice(&0u64.to_le_bytes()),
                5 => output.bytes[linkedit_offset + 24..linkedit_offset + 32]
                    .copy_from_slice(&output.final_image_layout.data_address.to_le_bytes()),
                6 => {
                    let bss_file_offset = wide(&output.bytes, data_offset + 40) as usize
                        + (output.final_image_layout.bss_address
                            - output.final_image_layout.data_address)
                            as usize;
                    output.bytes[bss_file_offset] = 1;
                }
                7 => {
                    let file_offset = wide(&output.bytes, data_offset + 40) as usize;
                    output.bytes[file_offset] ^= 1;
                }
                8 => output.bytes[data_offset + 32..data_offset + 40]
                    .copy_from_slice(&u64::MAX.to_le_bytes()),
                _ => output.bytes[text_offset + 68..text_offset + 72]
                    .copy_from_slice(&1u32.to_le_bytes()),
            }
            assert!(validate_mapping(&output).is_err(), "mutation {mutation}");
        }
    }

    fn code_signature_identifier(bytes: &[u8]) -> String {
        super::code_signature_identifier(bytes).expect("one signed CodeDirectory identifier")
    }

    #[test]
    fn authored_identifier_is_bound_into_the_code_directory() {
        let fallback = super::emit_macho_executable(storage_image(13, 24, 16))
            .expect("emit leaf-fallback image");
        assert_eq!(code_signature_identifier(&fallback.bytes), "omega-program");
        assert_eq!(fallback.file_name, "omega-program");
        let signed =
            super::emit_macho_executable_signed(storage_image(13, 24, 16), "com.omega.window-app")
                .expect("emit authored-identifier image");
        assert_eq!(
            code_signature_identifier(&signed.bytes),
            "com.omega.window-app"
        );
        assert_ne!(
            fallback.bytes, signed.bytes,
            "the signing identity is part of the emitted bytes"
        );
        let repeated =
            super::emit_macho_executable_signed(storage_image(13, 24, 16), "com.omega.window-app")
                .expect("re-emit authored-identifier image");
        assert_eq!(
            signed.bytes, repeated.bytes,
            "signed emission is deterministic for one bound identifier"
        );
        assert!(
            super::emit_macho_executable_signed(storage_image(13, 24, 16), "").is_err(),
            "an empty signing identifier rejects before image emission"
        );
    }

    #[test]
    fn loader_mapping_rejects_malformed_command_envelopes() {
        let original =
            super::emit_macho_executable(storage_image(0, 24, 16)).expect("emit storage image");
        for (offset, value) in [
            (0, 0),
            (12, 1),
            (24, 0),
            (28, 1),
            (16, u32::MAX),
            (16, 1),
            (20, u32::MAX),
            (36, 0),
            (36, 7),
            (36, u32::MAX),
            (32, 0x8000_0034),
        ] {
            let mut output = original.clone();
            crate::codec_bytes::patch_encoded(&mut output.bytes, offset, value.to_le_bytes());
            assert!(
                validate_mapping(&output).is_err(),
                "offset {offset}, value {value}"
            );
        }
        for length in [0, 7, 31, 40, original.bytes.len() - 1] {
            let mut output = original.clone();
            output.bytes.truncate(length);
            assert!(
                validate_mapping(&output).is_err(),
                "truncated length {length}"
            );
        }
    }
}

pub use dyld_linking::imports::validate_macho_import_binding_pairing;
use dyld_linking::imports::{
    install_import_thunks, macho_bind_info_for_segment, patch_import_thunks,
    validate_import_thunk_footprints,
};
use dyld_linking::load_commands::{
    write_empty_macho_dysymtab_command, write_empty_macho_symtab_command,
    write_macho_code_signature_command, write_macho_dyld_info_command, write_macho_dylib_header,
    write_macho_executable_build_version_command, write_macho_executable_data_segment,
    write_macho_executable_header, write_macho_executable_text_segment,
    write_macho_exports_trie_command, write_macho_id_dylib_command, write_macho_linkedit_segment,
    write_macho_load_dylib_command, write_macho_load_dylinker_command, write_macho_main_command,
    write_macho_pagezero_segment, write_macho_uuid_command,
};
pub use dyld_linking::loader_fixups::{
    MachoImportPointer, MachoRebasePointer, validate_macho_object_fixups,
};
pub use dyld_linking::loader_mapping::validate_macho_loader_mapping;
use dyld_linking::rebases::macho_rebase_info_for_segment;
use entry_symbol::macho_entry_text_offset;
use isa::MachoIsa;

const OMEGA_EXECUTABLE_LEAF: &str = "omega-program";

pub fn emit_macho_executable(image: FinalImage) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_macho_executable_signed(image, OMEGA_EXECUTABLE_LEAF)
}

pub fn emit_macho_executable_signed(
    image: FinalImage,
    code_signature_identifier: &str,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let isa = isa::macho_isa_for(image.target)?;
    let MachoContainerContents {
        image,
        bytes,
        layout,
        executable_regions,
        data_regions,
    } = emit_macho_container_contents(image, code_signature_identifier, isa, None, &[])?;
    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        final_import_data_bytes: Vec::new(),
        bytes,
        file_name: OMEGA_EXECUTABLE_LEAF.to_owned(),
        format: isa.format_name().to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions: PlacedDataRegionInventory::empty(),
    })
}
struct MachoContainerContents {
    image: FinalImage,
    bytes: Vec<u8>,
    layout: image::FinalImageLayout,
    executable_regions: image::PlacedExecutableRegionInventory,
    data_regions: image::PlacedDataRegionInventory,
}

fn emit_macho_container_contents(
    mut image: FinalImage,
    code_signature_identifier: &str,
    isa: MachoIsa,
    identity: Option<&dyld_linking::load_commands::MachoDylib>,
    exports: &[MachoComponentExport],
) -> Result<MachoContainerContents, Diagnostic> {
    if identity.is_some() != !exports.is_empty() {
        return Err(Diagnostic::error(
            "Mach-O container kind differs from its exact export roster",
        ));
    }
    if code_signature_identifier.is_empty() {
        return Err(Diagnostic::error(
            "Mach-O code signature identifier is empty",
        ));
    }
    if MachoIsa::from_format(image.target.object_format, image.target.architecture) != Some(isa) {
        return Err(Diagnostic::error(format!(
            "Mach-O {} emission cannot supply an image whose target is {:?}",
            isa.format_name(),
            image.target,
        )));
    }
    let imports = install_import_thunks(&mut image, isa)?;
    let import_thunks = imports.thunks;
    let dylibs = imports.dylibs;
    let data_segment = file_layout::plan::macho_data_segment_index(&image, identity.is_some());
    let bind_info = macho_bind_info_for_segment(&import_thunks, data_segment.unwrap_or(0))?;
    let rebase_info = macho_rebase_info_for_segment(&image, data_segment.unwrap_or(0))?;
    let mut plan = file_layout::plan::plan_macho_container(
        &image,
        import_thunks.len(),
        rebase_info.bytes.len(),
        bind_info.len(),
        &dylibs,
        code_signature_identifier,
        isa,
        identity,
        0,
    );
    let export_bytes = shared_component::export_trie(&image, exports, plan.text_offset)?;
    if identity.is_some() {
        let text_offset = plan.text_offset;
        plan = file_layout::plan::plan_macho_container(
            &image,
            import_thunks.len(),
            rebase_info.bytes.len(),
            bind_info.len(),
            &dylibs,
            code_signature_identifier,
            isa,
            identity,
            export_bytes.len(),
        );
        if plan.text_offset != text_offset || plan.data_segment_index != data_segment {
            return Err(Diagnostic::error(
                "Mach-O export layout changed its admitted segment geometry",
            ));
        }
    }
    let entry_offset = if identity.is_none() {
        Some(plan.text_offset + macho_entry_text_offset(&image)?)
    } else {
        None
    };
    let layout = plan.final_image_layout();

    patch_import_thunks(&mut image, &layout, &import_thunks, isa)?;
    image::apply_relocations(&mut image, &layout, "Mach-O direct executable")?;
    let rebase_pointers = rebase_info.validate_patched_preferred_pointers(&image, &layout)?;
    validate_import_thunk_footprints(&mut image, &import_thunks, isa)?;
    let executable_regions = place_executable_regions(&image, layout)?;
    let data_regions = place_data_regions(&image, layout)?;

    let mut bytes = Vec::new();
    if identity.is_some() {
        write_macho_dylib_header(&mut bytes, plan.command_count, plan.sizeofcmds, isa);
    } else {
        write_macho_executable_header(&mut bytes, plan.command_count, plan.sizeofcmds, isa);
        write_macho_pagezero_segment(&mut bytes);
    }
    write_macho_executable_text_segment(
        &mut bytes,
        plan.text_offset,
        image.memory.text.len(),
        plan.text_file_size,
        isa.page_size(),
    );
    if plan.has_data_segment {
        write_macho_executable_data_segment(
            &mut bytes,
            plan.data_offset,
            image.memory.data.len(),
            plan.bss_address,
            image.memory.bss_size,
            plan.data_vm_size,
            image.memory.bss_alignment,
        );
    }
    if identity.is_none() {
        write_macho_load_dylinker_command(&mut bytes);
    }
    write_macho_uuid_command(&mut bytes);
    write_macho_executable_build_version_command(&mut bytes);
    if let Some(entry_offset) = entry_offset {
        write_macho_main_command(&mut bytes, entry_offset);
    }
    if let Some(identity) = identity {
        write_macho_id_dylib_command(&mut bytes, identity);
    }
    for dylib in &dylibs {
        write_macho_load_dylib_command(&mut bytes, dylib);
    }
    if plan.has_dyld_info {
        write_macho_dyld_info_command(
            &mut bytes,
            plan.rebase_offset,
            rebase_info.bytes.len(),
            plan.bind_offset,
            bind_info.len(),
        );
    }
    if identity.is_some() {
        write_macho_exports_trie_command(&mut bytes, plan.export_offset, export_bytes.len());
    }
    write_macho_linkedit_segment(
        &mut bytes,
        plan.linkedit_vmaddr,
        plan.linkedit_offset,
        plan.linkedit_filesize,
        plan.linkedit_vmsize,
    );
    write_empty_macho_symtab_command(&mut bytes);
    write_empty_macho_dysymtab_command(&mut bytes);
    write_macho_code_signature_command(
        &mut bytes,
        plan.code_signature_offset,
        plan.code_signature_size,
    );
    bytes.resize(plan.text_offset, 0);
    bytes.extend(&image.memory.text);
    if plan.has_data_segment {
        bytes.resize(plan.data_offset, 0);
        bytes.extend(&image.memory.data);
    }
    bytes.resize(plan.rebase_offset, 0);
    bytes.extend(&rebase_info.bytes);
    bytes.resize(plan.bind_offset, 0);
    bytes.extend(bind_info);
    bytes.resize(plan.export_offset, 0);
    bytes.extend(&export_bytes);
    bytes.resize(plan.code_signature_offset, 0);
    let code_signature = code_signature::macho_ad_hoc_code_signature_for_container(
        &bytes,
        plan.text_file_size,
        code_signature_identifier,
        isa,
        identity.is_none(),
    );
    bytes.extend(code_signature);

    dyld_linking::loader_mapping::validate_macho_container_mapping(
        &bytes,
        layout,
        &image.memory.text,
        &image.memory.data,
        image.memory.bss_size,
        isa,
        identity,
    )?;
    let import_pointers: Vec<_> = import_thunks
        .iter()
        .map(|thunk| MachoImportPointer {
            data_offset: thunk.data_offset,
            install_name: &thunk.library,
            symbol: &thunk.bind_symbol,
        })
        .collect();
    dyld_linking::loader_fixups::validate_macho_loader_fixups_for_segment(
        &bytes,
        &image.memory.data,
        &rebase_pointers,
        &import_pointers,
        data_segment.unwrap_or(0),
    )?;
    if identity.is_some() {
        shared_component::validate_export_trie(&bytes, &image, exports, plan.text_offset)?;
    }

    Ok(MachoContainerContents {
        image,
        bytes,
        layout,
        executable_regions,
        data_regions,
    })
}
