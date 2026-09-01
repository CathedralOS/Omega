use crate::layout::align_to;
use crate::load_commands::MachoDylib;
use omega_calling_conventions::{
    DARWIN_LIBOBJC_PATH, MachineRegister, MachineState, MachineStateSet, RegisterSet,
    StateFootprintEvidence, darwin_import_library,
};
use omega_image::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageImportPlan,
    FinalImageLayout, FinalImageSection, FinalImageSymbolHandle,
};
use omega_object_file::SymbolKind;
use omega_target::{ForeignLocatorCandidate, NormalizedForeignLocator, TargetProfile};
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachoImportThunk {
    /// Object-local spelling retained only for diagnostics and region labels.
    pub(crate) symbol: String,
    /// Exact dyld symbol spelling. This may not be UTF-8 and is never rebuilt
    /// from `symbol` for normalized imports.
    pub(crate) bind_symbol: Vec<u8>,
    pub(crate) text_offset: usize,
    pub(crate) data_offset: usize,
    /// The install name of the dylib this symbol binds against — selects the
    /// bind-info dylib ordinal (load-command roster position + 1).
    pub(crate) library: Vec<u8>,
    pub(crate) dylib_ordinal: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledMachoImports {
    pub(crate) thunks: Vec<MachoImportThunk>,
    pub(crate) dylibs: Vec<MachoDylib>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedMachoImport {
    symbol_handle: FinalImageSymbolHandle,
    symbol: String,
    bind_symbol: Vec<u8>,
    library: Vec<u8>,
    legacy_objc_registration: bool,
}

fn ensure_dylib(dylibs: &mut Vec<MachoDylib>, path: &[u8]) {
    if !dylibs.iter().any(|dylib| dylib.path.as_ref() == path) {
        dylibs.push(MachoDylib::from_install_name(path.to_vec()));
    }
}

/// Derive the exact image-local dylib roster before any image state is changed.
/// libSystem remains ordinal 1 for compatibility; normalized install names are
/// de-duplicated by exact raw bytes in first-reference order.
fn plan_dylibs(imports: &[PreparedMachoImport]) -> Result<Vec<MachoDylib>, Diagnostic> {
    let mut dylibs = vec![MachoDylib::LIBSYSTEM];
    let uses_legacy_objc = imports.iter().any(|import| import.legacy_objc_registration);
    for import in imports {
        ensure_dylib(&mut dylibs, &import.library);
    }
    if uses_legacy_objc {
        ensure_dylib(&mut dylibs, MachoDylib::FOUNDATION.path.as_ref());
        ensure_dylib(&mut dylibs, MachoDylib::APPKIT.path.as_ref());
        ensure_dylib(&mut dylibs, MachoDylib::COREGRAPHICS.path.as_ref());
    }
    if dylibs.len() > 15 {
        return Err(Diagnostic::error(format!(
            "Mach-O image requires {} dylib ordinals, but the current canonical bind encoding supports at most 15",
            dylibs.len(),
        )));
    }
    Ok(dylibs)
}

pub(crate) fn install_import_thunks(
    image: &mut FinalImage,
) -> Result<InstalledMachoImports, Diagnostic> {
    // The host-ABI binding catalog registers an import symbol for EVERY binding,
    // but a program calls only a few. Only a REFERENCED import (a host-call `bl`
    // targets its thunk through a relocation) needs a thunk + bind entry; the rest
    // would be dead thunks whose bind entries force their dylibs
    // (libobjc/Foundation/AppKit/CoreGraphics) to load needlessly. Restrict to the
    // symbols an actual relocation points at, so e.g. a pure-filesystem program
    // links only libSystem.
    let referenced: Vec<_> = image
        .relocation_table
        .relocations
        .iter()
        .map(|(_, relocation)| relocation.symbol_handle)
        .filter(|handle| image.symbol_table.symbols.is_valid(*handle))
        .collect();
    let mut import_handles = Vec::new();
    let mut normalized_imports = Vec::<(FinalImageSymbolHandle, NormalizedForeignLocator)>::new();
    let mut prepared = Vec::new();
    for (_, import) in image.symbol_table.imports.iter() {
        if import_handles.contains(&import.symbol_handle) {
            return Err(Diagnostic::error(
                "Mach-O final image retains duplicate import rows for one symbol handle",
            ));
        }
        import_handles.push(import.symbol_handle);
        let symbol = image
            .symbol_table
            .symbols
            .is_valid(import.symbol_handle)
            .then(|| image.symbol_table.symbols.get(import.symbol_handle))
            .ok_or_else(|| {
                Diagnostic::error("Mach-O final image import names an invalid symbol handle")
            })?;
        if symbol.kind != SymbolKind::Import
            || symbol.section != FinalImageSection::None
            || symbol.offset != 0
            || symbol.size != 0
        {
            return Err(Diagnostic::error(format!(
                "Mach-O import `{}` is not an unresolved zero-width import symbol",
                symbol.name
            )));
        }
        let (library, bind_symbol, legacy_objc_registration) = match &import.import {
            FinalImageImportPlan::StringBackedBootstrap { .. } => {
                // Preserve the legacy bootstrap's target catalog mapping. Its
                // retained library string is a basename compatibility field,
                // not the raw LC_LOAD_DYLIB identity introduced by D45.
                let library = darwin_import_library(&symbol.name).as_bytes().to_vec();
                if symbol.name.is_empty() || symbol.name.as_bytes().contains(&0) {
                    return Err(Diagnostic::error(format!(
                        "Mach-O bootstrap import `{}` lacks a canonical library/symbol spelling",
                        symbol.name
                    )));
                }
                let uses_objc = library == DARWIN_LIBOBJC_PATH.as_bytes();
                (library, symbol.name.as_bytes().to_vec(), uses_objc)
            }
            FinalImageImportPlan::Normalized(locator) => {
                let ForeignLocatorCandidate::MachODylibSymbol {
                    install_name,
                    symbol,
                } = locator.locator()
                else {
                    return Err(Diagnostic::error(format!(
                        "normalized non-Mach-O foreign locator 0x{:016x} reached Mach-O image planning",
                        locator.non_authoritative_compatibility_fingerprint(),
                    )));
                };
                if locator.target() != TargetProfile::MacosArm64
                    || locator.target().native_target() != image.target
                {
                    return Err(Diagnostic::error(format!(
                        "Mach-O foreign locator 0x{:016x} targets `{}` but image planning targets {:?}",
                        locator.non_authoritative_compatibility_fingerprint(),
                        locator.target().target_name(),
                        image.target,
                    )));
                }
                if let Some((earlier_handle, earlier_locator)) = normalized_imports
                    .iter()
                    .find(|(_, earlier)| earlier.identity_digest() == locator.identity_digest())
                {
                    let detail = if earlier_locator == locator {
                        "the same exact locator is attached to more than one import symbol"
                    } else {
                        "distinct locators collide on one normalized identity"
                    };
                    return Err(Diagnostic::error(format!(
                        "Mach-O normalized import identity 0x{:016x} is ambiguous between symbol handles {:?} and {:?}: {detail}",
                        locator.non_authoritative_compatibility_fingerprint(),
                        earlier_handle,
                        import.symbol_handle,
                    )));
                }
                normalized_imports.push((import.symbol_handle, locator.clone()));
                (install_name.clone(), symbol.clone(), false)
            }
            FinalImageImportPlan::None => {
                return Err(Diagnostic::error(format!(
                    "Mach-O import `{}` has no retained physical import plan",
                    symbol.name
                )));
            }
        };
        if referenced.contains(&import.symbol_handle) {
            prepared.push(PreparedMachoImport {
                symbol_handle: import.symbol_handle,
                symbol: symbol.name.clone(),
                bind_symbol,
                library,
                legacy_objc_registration,
            });
        }
    }
    for (_, relocation) in image.relocation_table.relocations.iter() {
        if !image
            .symbol_table
            .symbols
            .is_valid(relocation.symbol_handle)
        {
            continue;
        }
        let symbol = image.symbol_table.symbols.get(relocation.symbol_handle);
        if symbol.kind == SymbolKind::Import && !import_handles.contains(&relocation.symbol_handle)
        {
            return Err(Diagnostic::error(format!(
                "Mach-O relocation references import `{}` without one exact import row",
                symbol.name
            )));
        }
    }

    // This is the transaction boundary: every import row, target, raw locator,
    // identity, and image-local ordinal is settled before any mutation below.
    let dylibs = plan_dylibs(&prepared)?;
    let mut thunks = Vec::with_capacity(prepared.len());
    for prepared_import in prepared {
        let symbol_handle = prepared_import.symbol_handle;
        let symbol = prepared_import.symbol;
        let text_offset = image.memory.text.len();
        image
            .memory
            .data
            .resize(align_to(image.memory.data.len(), 8), 0);
        let data_offset = image.memory.data.len();
        image.memory.text.extend([0u8; 12]);
        image.memory.data.extend([0u8; 8]);

        let image_symbol = image.symbol_table.symbols.get_mut(symbol_handle);
        image_symbol.section = FinalImageSection::Text;
        image_symbol.offset = text_offset;
        image_symbol.size = 12;

        image.executable_regions.push(FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::ImportThunk,
            section_offset: text_offset,
            byte_count: 12,
            symbol: symbol.clone(),
            footprint: None,
        });

        let dylib_ordinal = dylibs
            .iter()
            .position(|dylib| dylib.path.as_ref() == prepared_import.library)
            .map(|index| u8::try_from(index + 1).expect("preflighted Mach-O dylib ordinal"))
            .expect("preflighted Mach-O import library must be in load-command roster");
        thunks.push(MachoImportThunk {
            symbol,
            bind_symbol: prepared_import.bind_symbol,
            text_offset,
            data_offset,
            library: prepared_import.library,
            dylib_ordinal,
        });
    }

    Ok(InstalledMachoImports { thunks, dylibs })
}

pub(crate) fn patch_import_thunks(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    thunks: &[MachoImportThunk],
) -> Result<(), Diagnostic> {
    for thunk in thunks {
        let instruction_address = layout.text_address + thunk.text_offset as u64;
        let pointer_address = layout.data_address + thunk.data_offset as u64;
        patch_aarch64_adrp(
            &mut image.memory.text,
            thunk.text_offset,
            instruction_address,
            pointer_address,
        )?;
        patch_aarch64_ldr_x_from_page(
            &mut image.memory.text,
            thunk.text_offset + 4,
            pointer_address,
            16,
            16,
        )?;
        write_u32_at(&mut image.memory.text, thunk.text_offset + 8, 0xd61f_0200)?;
    }

    Ok(())
}

/// Validate the final patched Mach-O thunk opcode shape and attach its exact
/// architectural effect. The fixed sequence uses X16 as its sole scratch
/// register and transfers control without changing flags, stack, or vectors.
pub(crate) fn validate_import_thunk_footprints(
    image: &mut FinalImage,
    thunks: &[MachoImportThunk],
) -> Result<(), Diagnostic> {
    for thunk in thunks {
        let end = thunk.text_offset.checked_add(12).ok_or_else(|| {
            Diagnostic::error(format!(
                "Mach-O import thunk `{}` range overflows",
                thunk.symbol
            ))
        })?;
        let bytes = image
            .memory
            .text
            .get(thunk.text_offset..end)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "Mach-O import thunk `{}` is out of final .text bounds",
                    thunk.symbol
                ))
            })?;
        let adrp = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte ADRP"));
        let ldr = u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte LDR"));
        let br = u32::from_le_bytes(bytes[8..12].try_into().expect("four-byte BR"));
        if adrp & 0x9f00_001f != 0x9000_0010
            || ldr & 0xffc0_03ff != 0xf940_0210
            || br != 0xd61f_0200
        {
            return Err(Diagnostic::error(format!(
                "Mach-O import thunk `{}` does not match ADRP X16; LDR X16, [X16, #imm]; BR X16",
                thunk.symbol
            )));
        }
        let region = image
            .executable_regions
            .iter_mut()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::ImportThunk
                    && region.section_offset == thunk.text_offset
                    && region.byte_count == 12
                    && region.symbol == thunk.symbol
            })
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "Mach-O import thunk `{}` is missing its executable-region record",
                    thunk.symbol
                ))
            })?;
        region.footprint = Some(StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::Aarch64X(16)]),
            MachineStateSet::new([MachineState::InstructionPointer]),
        ));
    }
    Ok(())
}

pub(crate) fn macho_bind_info(thunks: &[MachoImportThunk]) -> Vec<u8> {
    let mut bytes = Vec::new();

    for thunk in thunks {
        // BIND_OPCODE_SET_DYLIB_ORDINAL_IMM | ordinal — which LC_LOAD_DYLIB this
        // symbol resolves through (1 = libSystem, 2 = libobjc, …). Ordinals ≤ 15
        // fit the immediate; the ordinal comes from `macho_dylib_list` order.
        debug_assert!(
            thunk.dylib_ordinal <= 0xf,
            "Mach-O dylib ordinal {} exceeds IMM",
            thunk.dylib_ordinal,
        );
        bytes.push(0x10 | thunk.dylib_ordinal);
        bytes.push(0x40); // SET_SYMBOL_TRAILING_FLAGS_IMM | 0
        bytes.extend(&thunk.bind_symbol);
        bytes.push(0);
        bytes.push(0x51); // SET_TYPE_IMM | BIND_TYPE_POINTER
        bytes.push(0x72); // SET_SEGMENT_AND_OFFSET_ULEB | segment 2 (__DATA)
        write_uleb128(&mut bytes, thunk.data_offset as u64);
        bytes.push(0x90); // DO_BIND
    }
    if !thunks.is_empty() {
        bytes.push(0);
    }

    bytes
}

fn write_uleb128(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn patch_aarch64_adrp(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let instruction_page = instruction_address & !0xfff;
    let symbol_page = symbol_address & !0xfff;
    let page_delta = (symbol_page as i64 - instruction_page as i64) / 4096;

    if !(-(1 << 20)..(1 << 20)).contains(&page_delta) {
        return Err(Diagnostic::error(format!(
            "Mach-O AArch64 import thunk ADRP is out of range: {page_delta} page(s)"
        )));
    }

    let immediate = (page_delta as u32) & 0x1f_ffff;
    let immediate_low = immediate & 0b11;
    let immediate_high = (immediate >> 2) & 0x7ffff;
    let instruction = 0x9000_0000 | (immediate_low << 29) | (immediate_high << 5) | 16;
    write_u32_at(text, offset, instruction)
}

fn patch_aarch64_ldr_x_from_page(
    text: &mut [u8],
    offset: usize,
    symbol_address: u64,
    register: u8,
    base_register: u8,
) -> Result<(), Diagnostic> {
    let page_offset = symbol_address & 0xfff;
    if !page_offset.is_multiple_of(8) {
        return Err(Diagnostic::error(
            "Mach-O AArch64 import thunk pointer is not 8-byte aligned",
        ));
    }
    let scaled_offset = u32::try_from(page_offset / 8)
        .expect("Mach-O AArch64 import thunk pointer offset overflow");
    if scaled_offset > 0xfff {
        return Err(Diagnostic::error(
            "Mach-O AArch64 import thunk pointer page offset is too large",
        ));
    }
    let instruction =
        0xf940_0000 | (scaled_offset << 10) | (u32::from(base_register) << 5) | u32::from(register);
    write_u32_at(text, offset, instruction)
}

fn write_u32_at(text: &mut [u8], offset: usize, value: u32) -> Result<(), Diagnostic> {
    let Some(slot) = text.get_mut(offset..offset + 4) else {
        return Err(Diagnostic::error(format!(
            "Mach-O AArch64 import thunk patch offset {offset} is out of bounds"
        )));
    };
    slot.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        install_import_thunks, macho_bind_info, patch_import_thunks,
        validate_import_thunk_footprints,
    };
    use crate::load_commands::write_macho_load_dylib_command;
    use omega_calling_conventions::MachineRegister;
    use omega_image::{
        FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageImportPlan,
        FinalImageLayout, FinalImageRelocation, FinalImageSymbol,
    };
    use omega_object_file::SymbolKind;
    use omega_target::{
        ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
    };
    use psi_arena::Handle;

    fn image_with_referenced_import() -> FinalImage {
        let mut image = FinalImage::with_capacity(
            NativeTarget::macos_arm64(),
            Default::default(),
            Handle::invalid(),
            1,
            1,
            1,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_write".into(),
            kind: SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::StringBackedBootstrap {
                library: "/usr/lib/libSystem.B.dylib".into(),
            },
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                symbol_handle: symbol,
                ..FinalImageRelocation::default()
            });
        image
    }

    fn image_with_normalized_import(
        install_name: Vec<u8>,
        bind_symbol: Vec<u8>,
        diagnostic_symbol: &str,
    ) -> FinalImage {
        let mut image = image_with_referenced_import();
        let symbol_handle = image
            .symbol_table
            .imports
            .iter()
            .next()
            .expect("one import symbol")
            .1
            .symbol_handle;
        image.symbol_table.symbols.get_mut(symbol_handle).name = diagnostic_symbol.into();
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name,
                symbol: bind_symbol,
            },
            TargetProfile::MacosArm64,
        )
        .expect("valid structural Mach-O locator");
        let import_handle = image
            .symbol_table
            .imports
            .iter()
            .next()
            .expect("one referenced import")
            .0;
        image.symbol_table.imports.get_mut(import_handle).import =
            FinalImageImportPlan::Normalized(locator);
        image
    }

    fn patch_test_thunks(image: &mut FinalImage, thunks: &[super::MachoImportThunk]) {
        patch_import_thunks(
            image,
            &FinalImageLayout {
                text_address: 0x1000,
                data_address: 0x2000,
                bss_address: 0x3000,
            },
            thunks,
        )
        .expect("test Mach-O thunk should patch");
    }

    #[test]
    fn installed_import_thunks_enter_the_executable_region_inventory() {
        let mut image = image_with_referenced_import();

        let imports = install_import_thunks(&mut image).expect("valid bootstrap import");
        patch_test_thunks(&mut image, &imports.thunks);
        validate_import_thunk_footprints(&mut image, &imports.thunks)
            .expect("patched Mach-O thunk bytes should validate");

        assert_eq!(imports.thunks.len(), 1);
        assert_eq!(image.executable_regions.len(), 1);
        assert_eq!(
            image.executable_regions[0].origin,
            FinalExecutableRegionOrigin::ImportThunk
        );
        assert_eq!(image.executable_regions[0].byte_count, 12);
        assert_eq!(image.executable_regions[0].symbol, "_write");
        let footprint = image.executable_regions[0]
            .footprint
            .as_ref()
            .expect("validated thunk should carry footprint evidence");
        assert!(
            footprint
                .registers()
                .contains(MachineRegister::Aarch64X(16))
        );
    }

    #[test]
    fn mutated_import_thunk_opcode_rejects_final_validation() {
        let mut image = image_with_referenced_import();
        let imports = install_import_thunks(&mut image).expect("valid bootstrap import");
        patch_test_thunks(&mut image, &imports.thunks);
        image.memory.text[9] = 0;

        let diagnostic = validate_import_thunk_footprints(&mut image, &imports.thunks)
            .expect_err("mutated Mach-O thunk opcode must reject");
        assert!(diagnostic.message.contains("ADRP X16"));
    }

    #[test]
    fn normalized_macho_locator_emits_exact_raw_load_and_bind_bytes() {
        let install_name = b"/tmp/lib\xffomega.dylib".to_vec();
        let bind_symbol = b"_raw_\xfe_entry".to_vec();
        let mut image = image_with_normalized_import(
            install_name.clone(),
            bind_symbol.clone(),
            "unrelated diagnostic label",
        );

        let imports = install_import_thunks(&mut image).expect("normalized Mach-O import");

        assert_eq!(imports.thunks.len(), 1);
        assert_eq!(imports.thunks[0].symbol, "unrelated diagnostic label");
        assert_eq!(imports.thunks[0].bind_symbol, bind_symbol);
        assert_eq!(imports.thunks[0].library, install_name);
        assert_eq!(imports.thunks[0].dylib_ordinal, 2);
        assert_eq!(imports.dylibs.len(), 2);
        let mut load_command = Vec::new();
        write_macho_load_dylib_command(&mut load_command, &imports.dylibs[1]);
        assert_eq!(
            &load_command[24..24 + install_name.len()],
            install_name.as_slice()
        );
        let bind_info = macho_bind_info(&imports.thunks);
        let mut expected_prefix = vec![0x12, 0x40];
        expected_prefix.extend(&bind_symbol);
        expected_prefix.push(0);
        assert!(bind_info.starts_with(&expected_prefix));
    }

    #[test]
    fn normalized_macho_imports_deduplicate_raw_install_names_and_share_ordinal() {
        let install_name = b"/tmp/libomega-custom.dylib".to_vec();
        let mut image = image_with_normalized_import(
            install_name.clone(),
            b"_first".to_vec(),
            "first diagnostic",
        );
        let second_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "second diagnostic".into(),
            kind: SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        let second_locator = normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: install_name.clone(),
                symbol: b"_second".to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .expect("second normalized locator");
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: second_symbol,
            import: FinalImageImportPlan::Normalized(second_locator),
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                symbol_handle: second_symbol,
                ..FinalImageRelocation::default()
            });

        let imports = install_import_thunks(&mut image).expect("two normalized imports");

        assert_eq!(imports.dylibs.len(), 2);
        assert_eq!(imports.dylibs[1].path.as_ref(), install_name);
        assert_eq!(
            imports
                .thunks
                .iter()
                .map(|thunk| thunk.dylib_ordinal)
                .collect::<Vec<_>>(),
            vec![2, 2]
        );
    }

    #[test]
    fn wrong_normalized_locator_case_rejects_without_image_mutation() {
        let mut image = image_with_referenced_import();
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"KERNEL32.dll".to_vec(),
                export: b"ExitProcess".to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid PE locator");
        let import_handle = image.symbol_table.imports.iter().next().unwrap().0;
        image.symbol_table.imports.get_mut(import_handle).import =
            FinalImageImportPlan::Normalized(locator);
        let before = image.clone();

        let diagnostic = install_import_thunks(&mut image)
            .expect_err("non-Mach-O normalized locator must reject");

        assert!(diagnostic.message.contains("non-Mach-O"));
        assert_eq!(image, before);
    }

    #[test]
    fn duplicate_import_row_rejects_without_image_mutation() {
        let mut image = image_with_referenced_import();
        let duplicate = image.symbol_table.imports.iter().next().unwrap().1.clone();
        image.symbol_table.imports.insert(duplicate);
        let before = image.clone();

        let diagnostic =
            install_import_thunks(&mut image).expect_err("duplicate import row must reject");

        assert!(diagnostic.message.contains("duplicate import rows"));
        assert_eq!(image, before);
    }

    #[test]
    fn repeated_normalized_identity_rejects_without_image_mutation() {
        let mut image = image_with_normalized_import(
            b"/tmp/libomega.dylib".to_vec(),
            b"_same".to_vec(),
            "first",
        );
        let locator = image
            .symbol_table
            .imports
            .iter()
            .next()
            .and_then(|(_, import)| match &import.import {
                FinalImageImportPlan::Normalized(locator) => Some(locator.clone()),
                _ => None,
            })
            .unwrap();
        let second_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "second".into(),
            kind: SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: second_symbol,
            import: FinalImageImportPlan::Normalized(locator),
        });
        let before = image.clone();

        let diagnostic = install_import_thunks(&mut image)
            .expect_err("one normalized identity cannot name two import symbols");

        assert!(diagnostic.message.contains("same exact locator"));
        assert_eq!(image, before);
    }

    #[test]
    fn excessive_dylib_ordinals_reject_before_image_mutation() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::macos_arm64(),
            Default::default(),
            Handle::invalid(),
            15,
            15,
            15,
        );
        for index in 0..15 {
            let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("diagnostic-{index}"),
                kind: SymbolKind::Import,
                ..FinalImageSymbol::default()
            });
            let locator = normalize_foreign_locator(
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: format!("/tmp/libomega-{index}.dylib").into_bytes(),
                    symbol: format!("_entry_{index}").into_bytes(),
                },
                TargetProfile::MacosArm64,
            )
            .expect("valid distinct locator");
            image.symbol_table.imports.insert(FinalImageImport {
                symbol_handle: symbol,
                import: FinalImageImportPlan::Normalized(locator),
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    symbol_handle: symbol,
                    ..FinalImageRelocation::default()
                });
        }
        let before = image.clone();

        let diagnostic = install_import_thunks(&mut image)
            .expect_err("sixteen image-local dylib ordinals exceed IMM encoding");

        assert!(diagnostic.message.contains("supports at most 15"));
        assert_eq!(image, before);
    }
}
