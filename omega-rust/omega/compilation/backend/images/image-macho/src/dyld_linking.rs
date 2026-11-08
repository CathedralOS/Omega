/*
We own the macOS dynamic-loader (`dyld`) contract for Mach-O containers:
preflighting the referenced dylib roster, installing AArch64 and x86-64 import
thunks and `__DATA` pointer slots, encoding ULEB128 bind and PIE rebase opcode
streams, writing all Mach-O load commands, and replaying segment mappings and
loader fixups against the finished container bytes.
*/

pub(crate) mod imports {
    use crate::dyld_linking::load_commands::MachoDylib;
    use crate::dyld_linking::write_uleb128;
    use crate::file_layout::layout::align_to;
    use crate::isa::MachoIsa;
    use calling_conventions::{DARWIN_LIBOBJC_PATH, darwin_import_library};
    use diagnostics::Diagnostic;
    use image::{
        FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
        FinalImage, FinalImageImportPlan, FinalImageLayout, FinalImageSection,
        FinalImageSymbolHandle, PlacedDataRegionInventory, PlacedExecutableRegionInventory,
    };
    use object_file::SymbolKind;
    use target::{ForeignLocatorCandidate, NormalizedImportLocator};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct MachoImportThunk {
        pub(crate) symbol: String,
        pub(crate) bind_symbol: Vec<u8>,
        pub(crate) text_offset: usize,
        pub(crate) data_offset: usize,
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
        isa: MachoIsa,
    ) -> Result<InstalledMachoImports, Diagnostic> {
        let referenced: Vec<_> = image
            .relocation_table
            .relocations
            .iter()
            .map(|(_, relocation)| relocation.symbol_handle)
            .filter(|handle| image.symbol_table.symbols.is_valid(*handle))
            .collect();
        let mut import_handles = Vec::new();
        let mut normalized_imports =
            Vec::<(FinalImageSymbolHandle, NormalizedImportLocator)>::new();
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
                    let (library, binding_symbol) = match locator {
                        NormalizedImportLocator::Foreign(foreign) => {
                            let ForeignLocatorCandidate::MachODylibSymbol {
                                install_name,
                                symbol,
                            } = foreign.locator()
                            else {
                                return Err(Diagnostic::error(format!(
                                    "normalized non-Mach-O foreign locator 0x{:016x} reached Mach-O image planning",
                                    foreign.non_authoritative_compatibility_fingerprint(),
                                )));
                            };
                            (install_name.clone(), symbol.clone())
                        }
                        NormalizedImportLocator::Component(component) => (
                            format!("@loader_path/{}", component.image_filename()).into_bytes(),
                            component.export_symbol().into_bytes(),
                        ),
                    };
                    if locator.target() != isa.target_profile()
                        || locator.target().native_target() != image.target
                    {
                        return Err(Diagnostic::error(format!(
                            "Mach-O import locator 0x{:016x} targets `{}` but image planning targets {:?}",
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
                    (library, binding_symbol, false)
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
            if symbol.kind == SymbolKind::Import
                && !import_handles.contains(&relocation.symbol_handle)
            {
                return Err(Diagnostic::error(format!(
                    "Mach-O relocation references import `{}` without one exact import row",
                    symbol.name
                )));
            }
        }

        let dylibs = plan_dylibs(&prepared)?;
        let mut thunks = Vec::with_capacity(prepared.len());
        for prepared_import in prepared {
            let symbol_handle = prepared_import.symbol_handle;
            let symbol = prepared_import.symbol;
            let text_offset = image.memory.text.len();
            let unpadded_data_len = image.memory.data.len();
            image.memory.data.resize(align_to(unpadded_data_len, 8), 0);
            if image.memory.data.len() != unpadded_data_len {
                image.data_regions.push(FinalDataRegion {
                    origin: FinalDataRegionOrigin::AlignmentPadding,
                    section_offset: unpadded_data_len,
                    byte_count: image.memory.data.len() - unpadded_data_len,
                    symbol: String::new(),
                });
            }
            let data_offset = image.memory.data.len();
            let thunk_size = isa.import_thunk_size();
            image.memory.text.extend([0u8].repeat(thunk_size));
            image.memory.data.extend([0u8; 8]);

            let image_symbol = image.symbol_table.symbols.get_mut(symbol_handle);
            image_symbol.section = FinalImageSection::Text;
            image_symbol.offset = text_offset;
            image_symbol.size = thunk_size;

            image.executable_regions.push(FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: text_offset,
                byte_count: thunk_size,
                symbol: symbol.clone(),
                footprint: None,
                entry_effects: None,
            });
            image.data_regions.push(FinalDataRegion {
                origin: FinalDataRegionOrigin::ImportBindingSlot,
                section_offset: data_offset,
                byte_count: 8,
                symbol: symbol.clone(),
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
        isa: MachoIsa,
    ) -> Result<(), Diagnostic> {
        for thunk in thunks {
            let instruction_address = layout.text_address + thunk.text_offset as u64;
            let pointer_address = layout.data_address + thunk.data_offset as u64;
            match isa {
                MachoIsa::Aarch64 => {
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
                MachoIsa::X86_64 => {
                    let displacement = i64::try_from(pointer_address)
                        .ok()
                        .zip(i64::try_from(instruction_address + 6).ok())
                        .and_then(|(target, next)| target.checked_sub(next))
                        .ok_or_else(|| {
                            Diagnostic::error(format!(
                                "Mach-O x86-64 import thunk `{}` pointer address overflows",
                                thunk.symbol
                            ))
                        })?;
                    let displacement = i32::try_from(displacement).map_err(|_| {
                        Diagnostic::error(format!(
                            "Mach-O x86-64 import thunk `{}` binding slot is out of disp32 range",
                            thunk.symbol
                        ))
                    })?;
                    let Some(slot) = image
                        .memory
                        .text
                        .get_mut(thunk.text_offset..thunk.text_offset + 6)
                    else {
                        return Err(Diagnostic::error(format!(
                            "Mach-O x86-64 import thunk `{}` patch offset is out of bounds",
                            thunk.symbol
                        )));
                    };
                    slot[..2].copy_from_slice(&[0xff, 0x25]);
                    crate::codec_bytes::patch_encoded(slot, 2, displacement.to_le_bytes());
                }
            }
        }

        Ok(())
    }

    fn thunk_diagnostic(symbol: &str, detail: &str) -> Diagnostic {
        Diagnostic::error(format!("Mach-O import thunk `{symbol}` {detail}"))
    }

    pub(crate) fn validate_import_thunk_footprints(
        image: &mut FinalImage,
        thunks: &[MachoImportThunk],
        isa: MachoIsa,
    ) -> Result<(), Diagnostic> {
        let thunk_size = isa.import_thunk_size();
        for thunk in thunks {
            let end = thunk
                .text_offset
                .checked_add(thunk_size)
                .ok_or_else(|| thunk_diagnostic(&thunk.symbol, "range overflows"))?;
            let bytes = image
                .memory
                .text
                .get(thunk.text_offset..end)
                .ok_or_else(|| thunk_diagnostic(&thunk.symbol, "is out of final .text bounds"))?;
            let canonical = match isa {
                MachoIsa::Aarch64 => {
                    let adrp = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte ADRP"));
                    let ldr = u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte LDR"));
                    let br = u32::from_le_bytes(bytes[8..12].try_into().expect("four-byte BR"));
                    adrp & 0x9f00_001f == 0x9000_0010
                        && ldr & 0xffc0_03ff == 0xf940_0210
                        && br == 0xd61f_0200
                }
                MachoIsa::X86_64 => isa_x86_64::decode_x86_64_import_thunk(bytes).is_some(),
            };
            if !canonical {
                return Err(thunk_diagnostic(
                    &thunk.symbol,
                    "does not match the ISA's canonical binding-slot branch",
                ));
            }
            let region = image
                .executable_regions
                .iter_mut()
                .find(|region| {
                    region.origin == FinalExecutableRegionOrigin::ImportThunk
                        && region.section_offset == thunk.text_offset
                        && region.byte_count == thunk_size
                        && region.symbol == thunk.symbol
                })
                .ok_or_else(|| {
                    thunk_diagnostic(&thunk.symbol, "is missing its executable-region record")
                })?;
            region.footprint = Some(isa.thunk_footprint());
        }
        Ok(())
    }

    pub(crate) fn macho_bind_info_for_segment(
        thunks: &[MachoImportThunk],
        data_segment: u8,
    ) -> Result<Vec<u8>, Diagnostic> {
        if data_segment > 0xf {
            return Err(Diagnostic::error(
                "Mach-O bind segment index exceeds its four-bit immediate",
            ));
        }
        let mut bytes = Vec::new();

        for thunk in thunks {
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
            bytes.push(0x70 | data_segment); // SET_SEGMENT_AND_OFFSET_ULEB | exact __DATA index.
            write_uleb128(&mut bytes, thunk.data_offset as u64);
            bytes.push(0x90); // DO_BIND
        }
        if !thunks.is_empty() {
            bytes.push(0);
        }

        Ok(bytes)
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
        let instruction = 0xf940_0000
            | (scaled_offset << 10)
            | (u32::from(base_register) << 5)
            | u32::from(register);
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

    pub fn validate_macho_import_binding_pairing(
        final_text_bytes: &[u8],
        executable_regions: &PlacedExecutableRegionInventory,
        data_regions: &PlacedDataRegionInventory,
        target: target::NativeTarget,
    ) -> Result<(), Diagnostic> {
        let isa = crate::isa::macho_isa_for(target)?;
        let thunks: Vec<_> = executable_regions
            .regions
            .iter()
            .filter(|region| region.origin == FinalExecutableRegionOrigin::ImportThunk)
            .collect();
        let slots: Vec<_> = data_regions
            .regions
            .iter()
            .filter(|region| region.origin == FinalDataRegionOrigin::ImportBindingSlot)
            .collect();

        let mut bound = Vec::with_capacity(thunks.len());
        for thunk in &thunks {
            bound.push((
                thunk,
                thunk_bound_pointer_address(final_text_bytes, thunk, isa)?,
            ));
        }
        for (thunk, bound_address) in &bound {
            let matching = slots
                .iter()
                .filter(|slot| {
                    slot.symbol == thunk.symbol
                        && slot.address == *bound_address
                        && slot.byte_count == 8
                        && slot.address.checked_sub(data_regions.data_address)
                            == Some(slot.section_offset as u64)
                })
                .count();
            if matching != 1 {
                return Err(Diagnostic::error(format!(
                    "Mach-O import thunk `{}` must bind exactly one placed binding slot at its decoded pointer address; found {matching}",
                    thunk.symbol
                )));
            }
        }
        for slot in &slots {
            let matching = bound
                .iter()
                .filter(|(thunk, bound_address)| {
                    thunk.symbol == slot.symbol && *bound_address == slot.address
                })
                .count();
            if matching != 1 {
                return Err(Diagnostic::error(format!(
                    "Mach-O import binding slot `{}` must be loaded by exactly one placed thunk; found {matching}",
                    slot.symbol
                )));
            }
        }
        Ok(())
    }

    fn thunk_bound_pointer_address(
        final_text_bytes: &[u8],
        region: &image::PlacedExecutableRegion,
        isa: MachoIsa,
    ) -> Result<u64, Diagnostic> {
        let thunk_size = isa.import_thunk_size();
        let end = region
            .section_offset
            .checked_add(thunk_size)
            .ok_or_else(|| thunk_diagnostic(&region.symbol, "range overflows"))?;
        let bytes = final_text_bytes
            .get(region.section_offset..end)
            .ok_or_else(|| thunk_diagnostic(&region.symbol, "is out of final .text bounds"))?;
        match isa {
            MachoIsa::Aarch64 => {
                let adrp = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte ADRP"));
                let ldr = u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte LDR"));
                let br = u32::from_le_bytes(bytes[8..12].try_into().expect("four-byte BR"));
                if adrp & 0x9f00_001f != 0x9000_0010
                    || ldr & 0xffc0_03ff != 0xf940_0210
                    || br != 0xd61f_0200
                {
                    return Err(thunk_diagnostic(
                        &region.symbol,
                        "does not match ADRP X16; LDR X16, [X16, #imm]; BR X16",
                    ));
                }
                let immediate = i64::from((((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0b11));
                let page_delta = (immediate << 43) >> 43;
                let slot_page =
                    (region.address & !0xfff)
                        .checked_add_signed(page_delta.checked_mul(4096).ok_or_else(|| {
                            thunk_diagnostic(&region.symbol, "page delta overflows")
                        })?)
                        .ok_or_else(|| thunk_diagnostic(&region.symbol, "bound page overflows"))?;
                let scaled_offset = u64::from((ldr >> 10) & 0xfff);
                slot_page.checked_add(scaled_offset * 8).ok_or_else(|| {
                    thunk_diagnostic(&region.symbol, "bound pointer address overflows")
                })
            }
            MachoIsa::X86_64 => {
                let displacement =
                    isa_x86_64::decode_x86_64_import_thunk(bytes).ok_or_else(|| {
                        thunk_diagnostic(
                            &region.symbol,
                            "does not match JMP qword ptr [rip + disp32]",
                        )
                    })?;
                (region.address + 6)
                    .checked_add_signed(i64::from(displacement))
                    .ok_or_else(|| {
                        thunk_diagnostic(&region.symbol, "bound pointer address overflows")
                    })
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            install_import_thunks, macho_bind_info_for_segment, patch_import_thunks,
            validate_import_thunk_footprints,
        };
        use crate::dyld_linking::load_commands::write_macho_load_dylib_command;
        use crate::isa::MachoIsa;
        use arena::Handle;
        use calling_conventions::{MachineRegister, MachineState, MachineStateSet};
        use image::{
            FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageImportPlan,
            FinalImageLayout, FinalImageRelocation, FinalImageSymbol,
        };
        use object_file::SymbolKind;
        use target::{
            ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
        };

        const TEST_LAYOUT: FinalImageLayout = FinalImageLayout {
            text_address: 0x1000,
            data_address: 0x2000,
            bss_address: 0x3000,
        };

        fn insert_write_import(image: &mut FinalImage) -> Handle<FinalImageSymbol> {
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
            symbol
        }

        fn image_with_referenced_import() -> FinalImage {
            let mut image =
                crate::test_support::image(NativeTarget::macos_arm64(), Default::default());
            insert_write_import(&mut image);
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
                FinalImageImportPlan::Normalized((locator).into());
            image
        }

        fn patch_test_thunks(image: &mut FinalImage, thunks: &[super::MachoImportThunk]) {
            patch_import_thunks(image, &TEST_LAYOUT, thunks, MachoIsa::Aarch64)
                .expect("test Mach-O thunk should patch");
        }

        #[test]
        fn installed_import_thunks_enter_the_executable_region_inventory() {
            let mut image = image_with_referenced_import();

            let imports = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect("valid bootstrap import");
            patch_test_thunks(&mut image, &imports.thunks);
            validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::Aarch64)
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
            let imports = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect("valid bootstrap import");
            patch_test_thunks(&mut image, &imports.thunks);
            image.memory.text[9] = 0;

            let diagnostic =
                validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::Aarch64)
                    .expect_err("mutated Mach-O thunk opcode must reject");
            assert!(diagnostic.message.contains("canonical binding-slot branch"));
        }

        #[test]
        fn x86_64_import_thunks_emit_and_validate_the_closed_jmp_rip_form() {
            let mut image =
                crate::test_support::image(NativeTarget::macos_x64(), Default::default());
            insert_write_import(&mut image);

            let imports = install_import_thunks(&mut image, MachoIsa::X86_64)
                .expect("valid x86-64 bootstrap import");
            assert_eq!(imports.thunks.len(), 1);
            assert_eq!(image.executable_regions[0].byte_count, 6);
            assert_eq!(image.memory.text.len(), 6);
            assert_eq!(image.memory.data.len(), 8);

            patch_import_thunks(&mut image, &TEST_LAYOUT, &imports.thunks, MachoIsa::X86_64)
                .expect("x86-64 Mach-O thunk should patch");
            assert_eq!(&image.memory.text[..], &[0xff, 0x25, 0xfa, 0x0f, 0, 0]);

            validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::X86_64)
                .expect("patched x86-64 thunk bytes should validate");
            let footprint = image.executable_regions[0]
                .footprint
                .as_ref()
                .expect("validated thunk should carry footprint evidence");
            assert!(!footprint.registers().contains(MachineRegister::X86Rax));
            assert!(
                footprint
                    .machine_state()
                    .contains_all(MachineStateSet::new([MachineState::InstructionPointer]))
            );
        }

        #[test]
        fn x86_64_mutated_import_thunk_rejects_final_validation() {
            let mut image =
                crate::test_support::image(NativeTarget::macos_x64(), Default::default());
            insert_write_import(&mut image);
            let imports = install_import_thunks(&mut image, MachoIsa::X86_64)
                .expect("valid x86-64 bootstrap import");
            patch_import_thunks(&mut image, &TEST_LAYOUT, &imports.thunks, MachoIsa::X86_64)
                .expect("patch");
            image.memory.text[0] = 0xff;
            image.memory.text[1] = 0x24;

            assert!(
                validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::X86_64)
                    .is_err()
            );
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

            let imports = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect("normalized Mach-O import");

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
            let bind_info = macho_bind_info_for_segment(&imports.thunks, 2)
                .expect("executable DATA segment fits");
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
                import: FinalImageImportPlan::Normalized((second_locator).into()),
            });
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    symbol_handle: second_symbol,
                    ..FinalImageRelocation::default()
                });

            let imports = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect("two normalized imports");

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
                FinalImageImportPlan::Normalized((locator).into());
            let before = image.clone();

            let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
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

            let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect_err("duplicate import row must reject");

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

            let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect_err("one normalized identity cannot name two import symbols");

            assert!(diagnostic.message.contains("same exact locator"));
            assert_eq!(image, before);
        }

        #[test]
        fn excessive_dylib_ordinals_reject_before_image_mutation() {
            let mut image =
                crate::test_support::image(NativeTarget::macos_arm64(), Default::default());
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
                    import: FinalImageImportPlan::Normalized((locator).into()),
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

            let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
                .expect_err("sixteen image-local dylib ordinals exceed IMM encoding");

            assert!(diagnostic.message.contains("supports at most 15"));
            assert_eq!(image, before);
        }
    }
}

pub(crate) mod load_commands {
    mod dynamic_linking {
        use crate::file_layout::bytes::{write_u32, write_u64};
        use crate::file_layout::constants::{
            MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE, MACHO_LOAD_DYLINKER_COMMAND_SIZE,
            MACHO_MAIN_COMMAND_SIZE,
        };
        use crate::file_layout::layout::align_to;
        use std::borrow::Cow;

        pub(crate) fn write_macho_load_dylinker_command(bytes: &mut Vec<u8>) {
            let start = bytes.len();
            write_u32(bytes, 0xe);
            write_u32(bytes, MACHO_LOAD_DYLINKER_COMMAND_SIZE as u32);
            write_u32(bytes, 12);
            bytes.extend(b"/usr/lib/dyld\0");
            bytes.resize(start + MACHO_LOAD_DYLINKER_COMMAND_SIZE, 0);
        }

        pub(crate) fn write_macho_main_command(bytes: &mut Vec<u8>, entry_offset: usize) {
            write_u32(bytes, 0x80000028);
            write_u32(bytes, MACHO_MAIN_COMMAND_SIZE as u32);
            write_u64(
                bytes,
                u64::try_from(entry_offset).expect("Mach-O entry offset overflow"),
            );
            write_u64(bytes, 0);
        }

        pub(crate) fn write_macho_executable_build_version_command(bytes: &mut Vec<u8>) {
            write_u32(bytes, 0x32);
            write_u32(bytes, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE as u32);
            write_u32(bytes, 1);
            write_u32(bytes, 13 << 16);
            write_u32(bytes, 13 << 16);
            write_u32(bytes, 1);
            write_u32(bytes, 3);
            write_u32(bytes, 0);
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct MachoDylib {
            pub(crate) path: Cow<'static, [u8]>,
            pub(crate) timestamp: u32,
            pub(crate) current_version: u32,
            pub(crate) compatibility_version: u32,
        }

        impl MachoDylib {
            const fn weak_floor(path: &'static [u8]) -> Self {
                MachoDylib {
                    path: Cow::Borrowed(path),
                    timestamp: 2,
                    current_version: 1 << 16,
                    compatibility_version: 1 << 16,
                }
            }

            pub(crate) const LIBSYSTEM: MachoDylib = MachoDylib {
                path: Cow::Borrowed(b"/usr/lib/libSystem.B.dylib"),
                timestamp: 2,
                current_version: 1351 << 16,
                compatibility_version: 1 << 16,
            };
            pub(crate) const LIBOBJC: MachoDylib =
                MachoDylib::weak_floor(b"/usr/lib/libobjc.A.dylib");
            pub(crate) const FOUNDATION: MachoDylib = MachoDylib::weak_floor(
                b"/System/Library/Frameworks/Foundation.framework/Versions/C/Foundation",
            );
            pub(crate) const APPKIT: MachoDylib = MachoDylib::weak_floor(
                b"/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit",
            );
            pub(crate) const COREGRAPHICS: MachoDylib = MachoDylib::weak_floor(
                b"/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics",
            );

            pub(crate) fn command_size(&self) -> usize {
                align_to(24 + self.path.len() + 1, 8)
            }

            pub(crate) fn from_install_name(path: Vec<u8>) -> Self {
                for known in [
                    Self::LIBSYSTEM,
                    Self::LIBOBJC,
                    Self::FOUNDATION,
                    Self::APPKIT,
                    Self::COREGRAPHICS,
                ] {
                    if known.path.as_ref() == path {
                        return known;
                    }
                }
                Self {
                    path: Cow::Owned(path),
                    timestamp: 2,
                    current_version: 1 << 16,
                    compatibility_version: 1 << 16,
                }
            }
        }

        pub(crate) fn write_macho_load_dylib_command(bytes: &mut Vec<u8>, dylib: &MachoDylib) {
            write_macho_dylib_command(bytes, dylib, 0xc);
        }

        pub(crate) fn write_macho_id_dylib_command(bytes: &mut Vec<u8>, dylib: &MachoDylib) {
            write_macho_dylib_command(bytes, dylib, 0xd);
        }

        fn write_macho_dylib_command(bytes: &mut Vec<u8>, dylib: &MachoDylib, command: u32) {
            let start = bytes.len();
            let command_size = dylib.command_size();
            write_u32(bytes, command);
            write_u32(bytes, command_size as u32);
            write_u32(bytes, 24); // dylib.name str offset (immediately after the header)
            write_u32(bytes, dylib.timestamp);
            write_u32(bytes, dylib.current_version);
            write_u32(bytes, dylib.compatibility_version);
            bytes.extend(dylib.path.as_ref());
            bytes.push(0);
            bytes.resize(start + command_size, 0);
        }

        #[cfg(test)]
        mod tests {
            use super::{MachoDylib, write_macho_load_dylib_command};

            #[test]
            fn libsystem_command_size_matches_historical_constant() {
                assert_eq!(MachoDylib::LIBSYSTEM.command_size(), 56);
            }

            #[test]
            fn dylib_command_is_self_describing_and_8_byte_aligned() {
                for dylib in [MachoDylib::LIBSYSTEM, MachoDylib::LIBOBJC] {
                    let mut bytes = Vec::new();
                    write_macho_load_dylib_command(&mut bytes, &dylib);
                    assert_eq!(bytes.len(), dylib.command_size());
                    assert_eq!(bytes.len() % 8, 0);
                    assert_eq!(
                        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize,
                        dylib.command_size()
                    );
                    assert_eq!(
                        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                        0xc
                    );
                    assert!(bytes[24..].starts_with(dylib.path.as_ref()));
                    assert_eq!(bytes[24 + dylib.path.len()], 0);
                }
            }
        }
    }

    mod header {
        use crate::file_layout::bytes::write_u32;
        use crate::file_layout::constants::{
            MACHO_HEADER_FLAGS_DYLDLINK, MACHO_HEADER_FLAGS_NOUNDEFS, MACHO_HEADER_FLAGS_PIE,
            MACHO_HEADER_FLAGS_TWOLEVEL,
        };
        use crate::isa::MachoIsa;

        pub(crate) fn write_macho_executable_header(
            bytes: &mut Vec<u8>,
            command_count: usize,
            sizeofcmds: usize,
            isa: MachoIsa,
        ) {
            write_macho_header_for(
                bytes,
                2,
                command_count,
                sizeofcmds,
                MACHO_HEADER_FLAGS_NOUNDEFS
                    | MACHO_HEADER_FLAGS_DYLDLINK
                    | MACHO_HEADER_FLAGS_TWOLEVEL
                    | MACHO_HEADER_FLAGS_PIE,
                isa,
            );
        }

        pub(crate) fn write_macho_dylib_header(
            bytes: &mut Vec<u8>,
            command_count: usize,
            sizeofcmds: usize,
            isa: MachoIsa,
        ) {
            write_macho_header_for(
                bytes,
                6,
                command_count,
                sizeofcmds,
                MACHO_HEADER_FLAGS_NOUNDEFS
                    | MACHO_HEADER_FLAGS_DYLDLINK
                    | MACHO_HEADER_FLAGS_TWOLEVEL,
                isa,
            );
        }

        pub(crate) fn write_macho_uuid_command(bytes: &mut Vec<u8>) {
            write_u32(bytes, 0x1b);
            write_u32(
                bytes,
                crate::file_layout::constants::MACHO_UUID_COMMAND_SIZE as u32,
            );
            bytes.extend([
                0x4f, 0x6d, 0x65, 0x67, 0x61, 0x2d, 0x64, 0x69, 0x72, 0x65, 0x63, 0x74, 0x2d, 0x6d,
                0x30, 0x31,
            ]);
        }

        fn write_macho_header_for(
            bytes: &mut Vec<u8>,
            file_type: u32,
            command_count: usize,
            sizeofcmds: usize,
            flags: u32,
            isa: MachoIsa,
        ) {
            write_u32(bytes, 0xfeedfacf);
            write_u32(bytes, isa.cpu_type());
            write_u32(bytes, isa.cpu_subtype());
            write_u32(bytes, file_type);
            write_u32(
                bytes,
                u32::try_from(command_count).expect("Mach-O command count overflow"),
            );
            write_u32(
                bytes,
                u32::try_from(sizeofcmds).expect("Mach-O load command size overflow"),
            );
            write_u32(bytes, flags);
            write_u32(bytes, 0);
        }
    }

    mod linkedit {
        use crate::file_layout::bytes::{write_reserved_u32s, write_u32};
        use crate::file_layout::constants::{
            MACHO_CODE_SIGNATURE_COMMAND_SIZE, MACHO_DYLD_INFO_COMMAND_SIZE,
            MACHO_DYSYMTAB_COMMAND_SIZE, MACHO_SYMTAB_COMMAND_SIZE,
        };

        pub(crate) fn write_macho_code_signature_command(
            bytes: &mut Vec<u8>,
            code_signature_offset: usize,
            code_signature_size: usize,
        ) {
            write_u32(bytes, 0x1d);
            write_u32(bytes, MACHO_CODE_SIGNATURE_COMMAND_SIZE as u32);
            write_u32(
                bytes,
                u32::try_from(code_signature_offset)
                    .expect("Mach-O code signature offset overflow"),
            );
            write_u32(
                bytes,
                u32::try_from(code_signature_size).expect("Mach-O code signature size overflow"),
            );
        }

        pub(crate) fn write_empty_macho_symtab_command(bytes: &mut Vec<u8>) {
            write_u32(bytes, 0x2);
            write_u32(bytes, MACHO_SYMTAB_COMMAND_SIZE as u32);
            write_reserved_u32s(bytes, 4);
        }

        pub(crate) fn write_empty_macho_dysymtab_command(bytes: &mut Vec<u8>) {
            write_u32(bytes, 0xb);
            write_u32(bytes, MACHO_DYSYMTAB_COMMAND_SIZE as u32);
            for _ in 0..18 {
                write_u32(bytes, 0);
            }
        }

        pub(crate) fn write_macho_dyld_info_command(
            bytes: &mut Vec<u8>,
            rebase_offset: usize,
            rebase_size: usize,
            bind_offset: usize,
            bind_size: usize,
        ) {
            write_u32(bytes, 0x8000_0022);
            write_u32(bytes, MACHO_DYLD_INFO_COMMAND_SIZE as u32);
            write_u32(
                bytes,
                u32::try_from(rebase_offset).expect("Mach-O rebase offset overflow"),
            );
            write_u32(
                bytes,
                u32::try_from(rebase_size).expect("Mach-O rebase size overflow"),
            );
            write_u32(
                bytes,
                u32::try_from(bind_offset).expect("Mach-O bind offset overflow"),
            );
            write_u32(
                bytes,
                u32::try_from(bind_size).expect("Mach-O bind size overflow"),
            );
            write_reserved_u32s(bytes, 6);
        }

        pub(crate) fn write_macho_exports_trie_command(
            bytes: &mut Vec<u8>,
            offset: usize,
            size: usize,
        ) {
            write_u32(bytes, 0x8000_0033);
            write_u32(bytes, 16);
            write_u32(
                bytes,
                u32::try_from(offset).expect("Mach-O export trie offset"),
            );
            write_u32(bytes, u32::try_from(size).expect("Mach-O export trie size"));
        }

        #[cfg(test)]
        mod tests {
            use super::write_macho_dyld_info_command;

            #[test]
            fn dyld_info_retains_rebase_and_bind_ranges_independently() {
                let mut bytes = Vec::new();
                write_macho_dyld_info_command(&mut bytes, 0x8000, 5, 0x8005, 0);

                let word =
                    |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                assert_eq!(word(0), 0x8000_0022);
                assert_eq!(word(8), 0x8000);
                assert_eq!(word(12), 5);
                assert_eq!(word(16), 0x8005);
                assert_eq!(word(20), 0);
            }
        }
    }

    mod segments {
        use crate::file_layout::bytes::{
            write_fixed_string_16, write_reserved_u32s, write_u32, write_u64,
        };
        use crate::file_layout::constants::{
            MACHO_EXECUTABLE_BASE, MACHO_SECTION_SIZE, MACHO_SEGMENT_COMMAND_SIZE,
        };
        use crate::file_layout::layout::{align_to_u64, alignment_power};

        macro_rules! u64 {
            ($a:expr => $b:expr) => {
                write_u64($a, $b)
            };
        }

        pub(crate) fn write_macho_pagezero_segment(bytes: &mut Vec<u8>) {
            write_u32(bytes, 0x19);
            write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
            write_fixed_string_16(bytes, "__PAGEZERO");
            write_u64(bytes, 0);
            write_u64(bytes, MACHO_EXECUTABLE_BASE);
            write_u64(bytes, 0);
            write_u64(bytes, 0);
            write_reserved_u32s(bytes, 4);
        }

        pub(crate) fn write_macho_executable_text_segment(
            bytes: &mut Vec<u8>,
            text_offset: usize,
            text_size: usize,
            text_file_size: usize,
            page_size: u64,
        ) {
            write_u32(bytes, 0x19);
            write_u32(
                bytes,
                u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
                    .expect("Mach-O text segment command size overflow"),
            );
            write_fixed_string_16(bytes, "__TEXT");
            write_u64(bytes, MACHO_EXECUTABLE_BASE);
            write_u64(bytes, align_to_u64(text_file_size as u64, page_size));
            write_u64(bytes, 0);
            u64!(bytes => u64::try_from(text_file_size).expect("Mach-O text file size overflow"));
            write_u32(bytes, 5);
            write_u32(bytes, 5);
            write_u32(bytes, 1);
            write_u32(bytes, 0);

            write_fixed_string_16(bytes, "__text");
            write_fixed_string_16(bytes, "__TEXT");
            write_u64(bytes, MACHO_EXECUTABLE_BASE + text_offset as u64);
            u64!(bytes => u64::try_from(text_size).expect("Mach-O text size overflow"));
            write_u32(
                bytes,
                u32::try_from(text_offset).expect("Mach-O text offset overflow"),
            );
            write_u32(bytes, 2);
            write_reserved_u32s(bytes, 2);
            write_u32(bytes, 0x80000400);
            write_reserved_u32s(bytes, 3);
        }

        pub(crate) fn write_macho_executable_data_segment(
            bytes: &mut Vec<u8>,
            data_offset: usize,
            data_size: usize,
            bss_address: u64,
            bss_size: usize,
            data_vm_size: u64,
            bss_alignment: usize,
        ) {
            let section_count = usize::from(data_size > 0) + usize::from(bss_size > 0);
            write_u32(bytes, 0x19);
            write_u32(
                bytes,
                u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + section_count * MACHO_SECTION_SIZE)
                    .expect("Mach-O data segment command size overflow"),
            );
            write_fixed_string_16(bytes, "__DATA");
            write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
            write_u64(bytes, data_vm_size);
            u64!(bytes => u64::try_from(data_offset).expect("Mach-O data file offset overflow"));
            u64!(bytes => u64::try_from(data_size).expect("Mach-O data file size overflow"));
            write_u32(bytes, 3);
            write_u32(bytes, 3);
            write_u32(
                bytes,
                u32::try_from(section_count).expect("Mach-O data section count overflow"),
            );
            write_u32(bytes, 0);

            if data_size > 0 {
                write_macho_executable_data_section(bytes, data_offset, data_size);
            }
            if bss_size > 0 {
                write_macho_executable_bss_section(bytes, bss_address, bss_size, bss_alignment);
            }
        }

        pub(crate) fn write_macho_linkedit_segment(
            bytes: &mut Vec<u8>,
            vmaddr: u64,
            file_offset: usize,
            file_size: usize,
            vm_size: usize,
        ) {
            write_u32(bytes, 0x19);
            write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
            write_fixed_string_16(bytes, "__LINKEDIT");
            write_u64(bytes, vmaddr);
            u64!(bytes => u64::try_from(vm_size).expect("Mach-O LINKEDIT vm size overflow"));
            u64!(bytes => u64::try_from(file_offset).expect("Mach-O LINKEDIT file offset overflow"));
            u64!(bytes => u64::try_from(file_size).expect("Mach-O LINKEDIT file size overflow"));
            write_u32(bytes, 1);
            write_u32(bytes, 1);
            write_reserved_u32s(bytes, 2);
        }

        fn write_macho_executable_data_section(
            bytes: &mut Vec<u8>,
            data_offset: usize,
            data_size: usize,
        ) {
            write_fixed_string_16(bytes, "__data");
            write_fixed_string_16(bytes, "__DATA");
            write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
            u64!(bytes => u64::try_from(data_size).expect("Mach-O data section size overflow"));
            write_u32(
                bytes,
                u32::try_from(data_offset).expect("Mach-O data section offset overflow"),
            );
            write_u32(bytes, 3);
            write_reserved_u32s(bytes, 6);
        }

        fn write_macho_executable_bss_section(
            bytes: &mut Vec<u8>,
            bss_address: u64,
            bss_size: usize,
            bss_alignment: usize,
        ) {
            write_fixed_string_16(bytes, "__bss");
            write_fixed_string_16(bytes, "__DATA");
            write_u64(bytes, bss_address);
            u64!(bytes => u64::try_from(bss_size).expect("Mach-O bss section size overflow"));
            write_u32(bytes, 0);
            write_u32(bytes, alignment_power(bss_alignment));
            write_reserved_u32s(bytes, 2);
            write_u32(bytes, 1);
            write_reserved_u32s(bytes, 3);
        }
    }

    pub(crate) use dynamic_linking::{
        MachoDylib, write_macho_executable_build_version_command, write_macho_id_dylib_command,
        write_macho_load_dylib_command, write_macho_load_dylinker_command,
        write_macho_main_command,
    };
    pub(crate) use header::{
        write_macho_dylib_header, write_macho_executable_header, write_macho_uuid_command,
    };
    pub(crate) use linkedit::{
        write_empty_macho_dysymtab_command, write_empty_macho_symtab_command,
        write_macho_code_signature_command, write_macho_dyld_info_command,
        write_macho_exports_trie_command,
    };
    pub(crate) use segments::{
        write_macho_executable_data_segment, write_macho_executable_text_segment,
        write_macho_linkedit_segment, write_macho_pagezero_segment,
    };
}

pub(crate) mod loader_fixups {
    use diagnostics::Diagnostic;
    use object_file::{
        ObjectPlan, RelocationKind, RelocationPlan, SectionKind, SymbolKind, SymbolSection,
    };

    use crate::dyld_linking::loader_mapping::{region, wide, word};

    #[derive(Clone, Copy)]
    pub struct MachoRebasePointer {
        pub data_offset: usize,
        pub preferred_address: u64,
    }

    #[derive(Clone, Copy)]
    pub struct MachoImportPointer<'inputs> {
        pub data_offset: usize,
        pub install_name: &'inputs [u8],
        pub symbol: &'inputs [u8],
    }

    pub fn validate_macho_object_fixups(
        object: &ObjectPlan,
        relocations: &RelocationPlan,
        object_text_bytes: usize,
        object_data_bytes: usize,
        output: &image::EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        let isa = crate::isa::macho_isa_for(object.target)?;
        if relocations.target != object.target {
            return Err(invalid());
        }
        let layout = output.final_image_layout;
        let referenced_imports = || {
            object.layout.symbols.iter().filter(|(handle, symbol)| {
                symbol.kind == SymbolKind::Import
                    && relocations
                        .records()
                        .any(|(_, relocation)| relocation.symbol_handle == *handle)
            })
        };
        let mut rebases = Vec::new();
        for (_, relocation) in relocations.records() {
            if relocation.kind != RelocationKind::Absolute64 {
                continue;
            }
            if relocation.section != SectionKind::Data
                || relocation.byte_width != 8
                || relocation
                    .offset
                    .checked_add(8)
                    .is_none_or(|end| end > object_data_bytes)
                || !object.layout.symbols.is_valid(relocation.symbol_handle)
            {
                return Err(invalid());
            }
            let symbol = object.layout.symbols.get(relocation.symbol_handle);
            let (base, offset) = if symbol.kind == SymbolKind::Import {
                let ordinal = referenced_imports()
                    .position(|(handle, _)| handle == relocation.symbol_handle)
                    .ok_or_else(invalid)?;
                let offset = ordinal
                    .checked_mul(isa.import_thunk_size())
                    .and_then(|offset| object_text_bytes.checked_add(offset))
                    .ok_or_else(invalid)?;
                (layout.text_address, offset)
            } else {
                let base = match symbol.section {
                    SymbolSection::Section(SectionKind::Text) => layout.text_address,
                    SymbolSection::Section(SectionKind::Data) => layout.data_address,
                    SymbolSection::Section(SectionKind::Bss) => layout.bss_address,
                    _ => return Err(invalid()),
                };
                (base, symbol.offset)
            };
            let preferred_address = base
                .checked_add(offset as u64)
                .and_then(|address| address.checked_add_signed(relocation.addend))
                .ok_or_else(invalid)?;
            rebases.push(MachoRebasePointer {
                data_offset: relocation.offset,
                preferred_address,
            });
        }
        rebases.sort_unstable_by_key(|pointer| pointer.data_offset);
        let mut imported_locations = Vec::new();
        let mut data_end = object_data_bytes;
        for (handle, symbol) in referenced_imports() {
            let mut locators = object
                .layout
                .normalized_imports
                .iter()
                .filter(|import| import.symbol == handle);
            let (install_name, bind_symbol) = if let Some(import) = locators.next() {
                if locators.next().is_some() || import.locator.target() != isa.target_profile() {
                    return Err(invalid());
                }
                match &import.locator {
                    target::NormalizedImportLocator::Foreign(locator) => {
                        let target::ForeignLocatorCandidate::MachODylibSymbol {
                            install_name,
                            symbol,
                        } = locator.locator()
                        else {
                            return Err(invalid());
                        };
                        (
                            std::borrow::Cow::Borrowed(install_name.as_slice()),
                            std::borrow::Cow::Borrowed(symbol.as_slice()),
                        )
                    }
                    target::NormalizedImportLocator::Component(component) => (
                        std::borrow::Cow::Owned(
                            format!("@loader_path/{}", component.image_filename()).into_bytes(),
                        ),
                        std::borrow::Cow::Owned(component.export_symbol().into_bytes()),
                    ),
                }
            } else {
                (
                    std::borrow::Cow::Borrowed(
                        calling_conventions::darwin_import_library(&symbol.name).as_bytes(),
                    ),
                    std::borrow::Cow::Borrowed(symbol.name.as_bytes()),
                )
            };
            let data_offset = data_end.checked_add(7).ok_or_else(invalid)? & !7;
            data_end = data_offset.checked_add(8).ok_or_else(invalid)?;
            imported_locations.push((data_offset, install_name, bind_symbol));
        }
        if data_end != output.final_data_bytes.len() {
            return Err(invalid());
        }
        let imports: Vec<_> = imported_locations
            .iter()
            .map(|(data_offset, install_name, symbol)| MachoImportPointer {
                data_offset: *data_offset,
                install_name: install_name.as_ref(),
                symbol: symbol.as_ref(),
            })
            .collect();
        validate_macho_loader_fixups_for_segment(
            &output.bytes,
            &output.final_data_bytes,
            &rebases,
            &imports,
            2,
        )
    }

    fn invalid() -> Diagnostic {
        Diagnostic::error("Mach-O loader fixups differ from the exact object pointer obligations")
    }

    struct Stream<'image> {
        remaining: &'image [u8],
    }

    impl<'image> Stream<'image> {
        fn byte(&mut self) -> Result<u8, Diagnostic> {
            let (byte, remaining) = self.remaining.split_first().ok_or_else(invalid)?;
            self.remaining = remaining;
            Ok(*byte)
        }

        fn require(&mut self, expected: u8) -> Result<(), Diagnostic> {
            if self.byte()? != expected {
                return Err(invalid());
            }
            Ok(())
        }

        fn unsigned(&mut self) -> Result<usize, Diagnostic> {
            let mut value = 0u64;
            for shift in (0..64).step_by(7) {
                let byte = self.byte()?;
                let payload = u64::from(byte & 0x7f);
                if payload > (u64::MAX >> shift) {
                    return Err(invalid());
                }
                value |= payload << shift;
                if byte & 0x80 == 0 {
                    if shift != 0 && payload == 0 {
                        return Err(invalid());
                    }
                    return usize::try_from(value).map_err(|_| invalid());
                }
            }
            Err(invalid())
        }

        fn terminated_bytes(&mut self) -> Result<&'image [u8], Diagnostic> {
            let length = self
                .remaining
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(invalid)?;
            let value = &self.remaining[..length];
            self.remaining = &self.remaining[length + 1..];
            Ok(value)
        }

        fn finish(mut self, has_operations: bool) -> Result<(), Diagnostic> {
            if has_operations {
                self.require(0)?;
            }
            if !self.remaining.is_empty() {
                return Err(invalid());
            }
            Ok(())
        }
    }

    pub(crate) fn validate_macho_loader_fixups_for_segment(
        bytes: &[u8],
        final_data: &[u8],
        rebases: &[MachoRebasePointer],
        imports: &[MachoImportPointer<'_>],
        expected_data_segment: u8,
    ) -> Result<(), Diagnostic> {
        if expected_data_segment > 0xf {
            return Err(invalid());
        }

        let commands = region(bytes, 32, word(bytes, 20)? as usize)?;
        let mut cursor = 0usize;
        let mut segment_count = 0usize;
        let mut data_segment = None;
        let mut linkedit = None;
        let mut signature = None;
        let mut dyld_info = None;
        let mut dylibs: [Option<&[u8]>; 15] = [None; 15];
        let mut dylib_count = 0usize;
        for _ in 0..word(bytes, 16)? {
            let command_size = word(commands, cursor.checked_add(4).ok_or_else(invalid)?)? as usize;
            if command_size < 8 || !command_size.is_multiple_of(8) {
                return Err(invalid());
            }
            let command = region(commands, cursor, command_size)?;
            match word(command, 0)? {
                0x19 => {
                    let name = region(command, 8, 16)?;
                    if name == b"__DATA\0\0\0\0\0\0\0\0\0\0"
                        && data_segment.replace(segment_count).is_some()
                    {
                        return Err(invalid());
                    }
                    if name == b"__LINKEDIT\0\0\0\0\0\0"
                        && linkedit
                            .replace((wide(command, 40)?, wide(command, 48)?))
                            .is_some()
                    {
                        return Err(invalid());
                    }
                    segment_count += 1;
                }
                0xc => {
                    if command_size < 32 || dylib_count == dylibs.len() || word(command, 8)? != 24 {
                        return Err(invalid());
                    }
                    let mut path = Stream {
                        remaining: region(command, 24, command_size - 24)?,
                    };
                    dylibs[dylib_count] = Some(path.terminated_bytes()?);
                    dylib_count += 1;
                }
                0x8000_0022 => {
                    if command_size != 48 || dyld_info.replace(command).is_some() {
                        return Err(invalid());
                    }
                }
                0x1d => {
                    if command_size != 16 || signature.is_some() {
                        return Err(invalid());
                    }
                    signature = Some(word(command, 8)? as usize);
                }
                _ => {}
            }
            cursor = cursor.checked_add(command_size).ok_or_else(invalid)?;
        }
        if cursor != commands.len() {
            return Err(invalid());
        }
        let Some(info) = dyld_info else {
            return if rebases.is_empty() && imports.is_empty() {
                Ok(())
            } else {
                Err(invalid())
            };
        };
        if info[24..].iter().any(|byte| *byte != 0)
            || data_segment != Some(usize::from(expected_data_segment))
        {
            return Err(invalid());
        }
        let (linkedit_offset, linkedit_size) = linkedit.ok_or_else(invalid)?;
        let linkedit_end = linkedit_offset
            .checked_add(linkedit_size)
            .ok_or_else(invalid)?;
        let rebase_offset = word(info, 8)? as usize;
        let rebase_size = word(info, 12)? as usize;
        let bind_offset = word(info, 16)? as usize;
        let bind_size = word(info, 20)? as usize;
        let fixup_end = bind_offset.checked_add(bind_size).ok_or_else(invalid)?;
        let rebase_end = rebase_offset.checked_add(rebase_size).ok_or_else(invalid)?;
        if rebase_offset as u64 != linkedit_offset
            || rebase_end > bind_offset
            || bind_offset - rebase_end >= 8
            || !bind_offset.is_multiple_of(8)
            || region(bytes, rebase_end, bind_offset - rebase_end)?
                .iter()
                .any(|byte| *byte != 0)
            || fixup_end > signature.ok_or_else(invalid)?
            || fixup_end as u64 > linkedit_end
        {
            return Err(invalid());
        }
        let mut rebase_stream = Stream {
            remaining: region(bytes, rebase_offset, rebase_size)?,
        };
        if !rebases.is_empty() {
            rebase_stream.require(0x11)?; // SET_TYPE POINTER.
        }
        let mut previous_end = 0usize;
        for pointer in rebases {
            validate_slot(final_data, pointer.data_offset, previous_end)?;
            rebase_stream.require(0x20 | expected_data_segment)?; // Exact __DATA index.
            if rebase_stream.unsigned()? != pointer.data_offset
                || wide(final_data, pointer.data_offset)? != pointer.preferred_address
            {
                return Err(invalid());
            }
            rebase_stream.require(0x51)?; // DO_REBASE once, eight-byte pointer.
            previous_end = pointer.data_offset + 8;
        }
        rebase_stream.finish(!rebases.is_empty())?;

        let mut bind_stream = Stream {
            remaining: region(bytes, bind_offset, bind_size)?,
        };
        for pointer in imports {
            validate_slot(final_data, pointer.data_offset, previous_end)?;
            let ordinal = bind_stream.byte()?;
            if ordinal & 0xf0 != 0x10 || ordinal & 0xf == 0 {
                return Err(invalid());
            }
            let install_name = dylibs[usize::from((ordinal & 0xf) - 1)].ok_or_else(invalid)?;
            bind_stream.require(0x40)?; // SET_SYMBOL, no weak/special flags.
            if install_name != pointer.install_name
                || bind_stream.terminated_bytes()? != pointer.symbol
                || pointer.symbol.is_empty()
                || wide(final_data, pointer.data_offset)? != 0
            {
                return Err(invalid());
            }
            bind_stream.require(0x51)?; // SET_TYPE POINTER.
            bind_stream.require(0x70 | expected_data_segment)?; // Exact __DATA index.
            if bind_stream.unsigned()? != pointer.data_offset {
                return Err(invalid());
            }
            bind_stream.require(0x90)?; // DO_BIND once, eight-byte pointer.
            previous_end = pointer.data_offset + 8;
        }
        bind_stream.finish(!imports.is_empty())
    }

    fn validate_slot(bytes: &[u8], offset: usize, previous_end: usize) -> Result<(), Diagnostic> {
        if offset < previous_end || !offset.is_multiple_of(8) {
            return Err(invalid());
        }
        region(bytes, offset, 8)?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::Stream;

        #[test]
        fn fixup_offsets_are_bounded_and_canonical() {
            for (encoded, expected) in [(&[0][..], 0), (&[0x7f][..], 127), (&[0x80, 1][..], 128)] {
                let mut stream = Stream { remaining: encoded };
                assert_eq!(stream.unsigned().unwrap(), expected);
                assert!(stream.remaining.is_empty());
            }
            for encoded in [
                &[0x80][..],
                &[0x80, 0][..],
                &[0xff, 0][..],
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 2][..],
                &[0x80; 11][..],
            ] {
                assert!(Stream { remaining: encoded }.unsigned().is_err());
            }
        }
    }
}

pub(crate) mod loader_mapping {
    use diagnostics::Diagnostic;
    use image::FinalImageLayout;

    use crate::file_layout::constants::MACHO_EXECUTABLE_BASE;
    use crate::isa::MachoIsa;

    fn invalid() -> Diagnostic {
        Diagnostic::error("Mach-O loader mapping differs from the supported final image")
    }

    pub(crate) fn region(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8], Diagnostic> {
        bytes
            .get(offset..offset.checked_add(count).ok_or_else(invalid)?)
            .ok_or_else(invalid)
    }

    pub(crate) fn word(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
        Ok(u32::from_le_bytes(
            region(bytes, offset, 4)?
                .try_into()
                .map_err(|_| invalid())?,
        ))
    }

    pub(crate) fn wide(bytes: &[u8], offset: usize) -> Result<u64, Diagnostic> {
        Ok(u64::from_le_bytes(
            region(bytes, offset, 8)?
                .try_into()
                .map_err(|_| invalid())?,
        ))
    }

    fn end(start: u64, count: u64) -> Result<u64, Diagnostic> {
        start.checked_add(count).ok_or_else(invalid)
    }

    fn aligned(value: u64, alignment: u64) -> Result<u64, Diagnostic> {
        Ok(end(value, alignment - 1)? & !(alignment - 1))
    }

    struct Segment<'bytes> {
        sections: &'bytes [u8],
        address: u64,
        memory_size: u64,
        file_offset: u64,
        file_size: u64,
    }

    impl<'bytes> Segment<'bytes> {
        fn read(
            command: &'bytes [u8],
            protection: u32,
            bytes: &[u8],
            page_size: u64,
        ) -> Result<Self, Diagnostic> {
            let count = usize::try_from(word(command, 64)?).map_err(|_| invalid())?;
            let section_bytes = count.checked_mul(80).ok_or_else(invalid)?;
            if command.len() != 72usize.checked_add(section_bytes).ok_or_else(invalid)?
                || word(command, 56)? != protection
                || word(command, 60)? != protection
                || word(command, 68)? != 0
            {
                return Err(invalid());
            }
            let segment = Self {
                sections: region(command, 72, section_bytes)?,
                address: wide(command, 24)?,
                memory_size: wide(command, 32)?,
                file_offset: wide(command, 40)?,
                file_size: wide(command, 48)?,
            };
            let page = page_size;
            if !segment.address.is_multiple_of(page)
                || !segment.memory_size.is_multiple_of(page)
                || !segment.file_offset.is_multiple_of(page)
                || segment.file_size > segment.memory_size
                || end(segment.file_offset, segment.file_size)? > bytes.len() as u64
            {
                return Err(invalid());
            }
            end(segment.address, segment.memory_size)?;
            Ok(segment)
        }
    }

    pub fn validate_macho_loader_mapping(
        bytes: &[u8],
        layout: FinalImageLayout,
        final_text: &[u8],
        final_data: &[u8],
        bss_bytes: usize,
        target: target::NativeTarget,
    ) -> Result<(), Diagnostic> {
        validate_macho_container_mapping(
            bytes,
            layout,
            final_text,
            final_data,
            bss_bytes,
            crate::isa::macho_isa_for(target)?,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_macho_container_mapping(
        bytes: &[u8],
        layout: FinalImageLayout,
        final_text: &[u8],
        final_data: &[u8],
        bss_bytes: usize,
        isa: MachoIsa,
        identity: Option<&crate::dyld_linking::load_commands::MachoDylib>,
    ) -> Result<(), Diagnostic> {
        let page_size = isa.page_size();
        if word(bytes, 0)? != 0xfeed_facf
            || word(bytes, 4)? != isa.cpu_type()
            || word(bytes, 8)? != isa.cpu_subtype()
            || word(bytes, 12)? != if identity.is_some() { 6 } else { 2 }
            || word(bytes, 24)? != if identity.is_some() { 0x85 } else { 0x20_0085 }
            || word(bytes, 28)? != 0
        {
            return Err(invalid());
        }
        let commands_size = usize::try_from(word(bytes, 20)?).map_err(|_| invalid())?;
        let commands = region(bytes, 32, commands_size)?;
        let command_count = usize::try_from(word(bytes, 16)?).map_err(|_| invalid())?;
        if command_count > commands.len() / 8 {
            return Err(invalid());
        }
        let mut segments: [Option<Segment<'_>>; 4] = [None, None, None, None];
        let mut singletons = 0u16;
        let mut dylib_count = 0usize;
        let mut entry_file_offset = None;
        let mut cursor = 0usize;
        for _ in 0..command_count {
            let size = usize::try_from(word(commands, cursor.checked_add(4).ok_or_else(invalid)?)?)
                .map_err(|_| invalid())?;
            if size < 8 || !size.is_multiple_of(8) {
                return Err(invalid());
            }
            let command = region(commands, cursor, size)?;
            match word(command, 0)? {
                0x19 => {
                    let (slot, protection) = match region(command, 8, 16)? {
                        b"__PAGEZERO\0\0\0\0\0\0" => (0, 0),
                        b"__TEXT\0\0\0\0\0\0\0\0\0\0" => (1, 5),
                        b"__DATA\0\0\0\0\0\0\0\0\0\0" => (2, 3),
                        b"__LINKEDIT\0\0\0\0\0\0" => (3, 1),
                        _ => return Err(invalid()),
                    };
                    if segments[slot]
                        .replace(Segment::read(command, protection, bytes, page_size)?)
                        .is_some()
                    {
                        return Err(invalid());
                    }
                }
                0xc => {
                    if size < 32 || word(command, 8)? != 24 || !command[24..].contains(&0) {
                        return Err(invalid());
                    }
                    dylib_count += 1;
                }
                0xd => {
                    let expected = identity.ok_or_else(invalid)?;
                    if singletons & (1 << 8) != 0
                        || size != expected.command_size()
                        || word(command, 8)? != 24
                        || word(command, 12)? != expected.timestamp
                        || word(command, 16)? != expected.current_version
                        || word(command, 20)? != expected.compatibility_version
                        || region(command, 24, expected.path.len())? != expected.path.as_ref()
                        || command[24 + expected.path.len()..]
                            .iter()
                            .any(|byte| *byte != 0)
                    {
                        return Err(invalid());
                    }
                    singletons |= 1 << 8;
                }
                0x8000_0033 => {
                    if identity.is_none() || size != 16 || singletons & (1 << 9) != 0 {
                        return Err(invalid());
                    }
                    let offset = usize::try_from(word(command, 8)?).map_err(|_| invalid())?;
                    let count = usize::try_from(word(command, 12)?).map_err(|_| invalid())?;
                    if count == 0 {
                        return Err(invalid());
                    }
                    region(bytes, offset, count)?;
                    singletons |= 1 << 9;
                }
                kind => {
                    let (bit, expected_size) = match kind {
                        0xe => (0, 32),
                        0x1b => (1, 24),
                        0x32 => (2, 32),
                        0x8000_0028 => (3, 24),
                        0x8000_0022 => (4, 48),
                        0x2 => (5, 24),
                        0xb => (6, 80),
                        0x1d => (7, 16),
                        _ => return Err(invalid()),
                    };
                    if (identity.is_some() && matches!(kind, 0xe | 0x8000_0028))
                        || size != expected_size
                        || singletons & (1 << bit) != 0
                    {
                        return Err(invalid());
                    }
                    singletons |= 1 << bit;
                    if kind == 0xe
                        && (word(command, 8)? != 12
                            || region(command, 12, 14)? != b"/usr/lib/dyld\0")
                    {
                        return Err(invalid());
                    }
                    if kind == 0x8000_0028 {
                        if wide(command, 16)? != 0 {
                            return Err(invalid());
                        }
                        entry_file_offset = Some(wide(command, 8)?);
                    }
                    if matches!(kind, 0x2 | 0xb) && command[8..].iter().any(|byte| *byte != 0) {
                        return Err(invalid());
                    }
                }
            }
            cursor = cursor.checked_add(size).ok_or_else(invalid)?;
        }
        let required_singletons = if identity.is_some() { 0x3e6 } else { 0xef };
        if cursor != commands.len()
            || singletons & required_singletons != required_singletons
            || dylib_count == 0
        {
            return Err(invalid());
        }
        let [pagezero, Some(text), data, Some(linkedit)] = segments else {
            return Err(invalid());
        };
        match (identity, pagezero) {
            (None, Some(pagezero))
                if pagezero.address == 0
                    && pagezero.memory_size == MACHO_EXECUTABLE_BASE
                    && pagezero.file_offset == 0
                    && pagezero.file_size == 0
                    && pagezero.sections.is_empty() => {}
            (Some(_), None) => {}
            _ => return Err(invalid()),
        }
        if text.address != MACHO_EXECUTABLE_BASE
            || text.file_offset != 0
            || text.memory_size != text.file_size
            || text.sections.len() != 80
        {
            return Err(invalid());
        }
        let text_file = layout
            .text_address
            .checked_sub(text.address)
            .ok_or_else(invalid)?;
        if text_file != aligned(end(32, commands_size as u64)?, 16)?
            || text.file_size != aligned(end(text_file, final_text.len() as u64)?, page_size)?
        {
            return Err(invalid());
        }
        validate_section(
            text.sections,
            b"__text\0\0\0\0\0\0\0\0\0\0",
            b"__TEXT\0\0\0\0\0\0\0\0\0\0",
            layout.text_address,
            final_text.len() as u64,
            text_file,
            0x8000_0400,
        )?;
        if word(text.sections, 52)? != 2 {
            return Err(invalid());
        }
        compare_file(bytes, text_file, final_text)?;
        if identity.is_none() {
            let entry = entry_file_offset.ok_or_else(invalid)?;
            if entry < text_file
                || entry >= end(text_file, final_text.len() as u64)?
                || !entry.is_multiple_of(isa.entry_alignment())
            {
                return Err(invalid());
            }
        }
        let text_end = end(text.address, text.memory_size)?;
        let mut mapped_end = text_end;
        let mut file_end = text.file_size;
        if let Some(data) = data {
            if final_data.is_empty() && bss_bytes == 0 {
                return Err(invalid());
            }
            if data.address != text_end
                || data.address != layout.data_address
                || data.file_offset != text.file_size
                || data.file_size != final_data.len() as u64
                || data.sections.len()
                    != 80 * (usize::from(!final_data.is_empty()) + usize::from(bss_bytes != 0))
            {
                return Err(invalid());
            }
            let mut sections = data.sections.as_chunks::<80>().0.iter();
            if !final_data.is_empty() {
                let section = sections.next().ok_or_else(invalid)?;
                validate_section(
                    section,
                    b"__data\0\0\0\0\0\0\0\0\0\0",
                    b"__DATA\0\0\0\0\0\0\0\0\0\0",
                    data.address,
                    data.file_size,
                    data.file_offset,
                    0,
                )?;
                if word(section, 52)? != 3 {
                    return Err(invalid());
                }
            }
            let initialized_end = end(data.address, data.file_size)?;
            let storage_end = if bss_bytes != 0 {
                let section = sections.next().ok_or_else(invalid)?;
                validate_section(
                    section,
                    b"__bss\0\0\0\0\0\0\0\0\0\0\0",
                    b"__DATA\0\0\0\0\0\0\0\0\0\0",
                    layout.bss_address,
                    bss_bytes as u64,
                    0,
                    1,
                )?;
                let alignment = 1u64.checked_shl(word(section, 52)?).ok_or_else(invalid)?;
                if layout.bss_address != aligned(initialized_end, alignment)? {
                    return Err(invalid());
                }
                let storage_end = end(layout.bss_address, bss_bytes as u64)?;
                let mapped_file_end = end(data.address, aligned(data.file_size, page_size)?)?;
                let tail_end = storage_end.min(mapped_file_end);
                if tail_end > layout.bss_address {
                    let tail_offset = end(data.file_offset, layout.bss_address - data.address)?;
                    let tail = region(
                        bytes,
                        usize::try_from(tail_offset).map_err(|_| invalid())?,
                        usize::try_from(tail_end - layout.bss_address).map_err(|_| invalid())?,
                    )?;
                    if tail.iter().any(|byte| *byte != 0) {
                        return Err(invalid());
                    }
                }
                storage_end
            } else {
                initialized_end
            };
            mapped_end = end(data.address, data.memory_size)?;
            let required_end = aligned(storage_end, page_size)?;
            if mapped_end < required_end || (bss_bytes != 0 && mapped_end != required_end) {
                return Err(invalid());
            }
            compare_file(bytes, data.file_offset, final_data)?;
            file_end = end(data.file_offset, data.file_size)?;
        } else if !final_data.is_empty() || bss_bytes != 0 {
            return Err(invalid());
        }
        if linkedit.address != mapped_end
            || !linkedit.sections.is_empty()
            || linkedit.file_offset != aligned(file_end, page_size)?
            || end(linkedit.file_offset, linkedit.file_size)? != bytes.len() as u64
            || linkedit.memory_size != aligned(linkedit.file_size, page_size)?
        {
            return Err(invalid());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_section(
        section: &[u8],
        name: &[u8; 16],
        segment: &[u8; 16],
        address: u64,
        size: u64,
        file_offset: u64,
        flags: u32,
    ) -> Result<(), Diagnostic> {
        if region(section, 0, 16)? != name
            || region(section, 16, 16)? != segment
            || wide(section, 32)? != address
            || wide(section, 40)? != size
            || u64::from(word(section, 48)?) != file_offset
            || word(section, 64)? != flags
            || word(section, 56)? != 0
            || word(section, 60)? != 0
            || section[68..].iter().any(|byte| *byte != 0)
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn compare_file(bytes: &[u8], offset: u64, expected: &[u8]) -> Result<(), Diagnostic> {
        if region(
            bytes,
            usize::try_from(offset).map_err(|_| invalid())?,
            expected.len(),
        )? != expected
        {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(crate) mod rebases {
    use super::write_uleb128;
    use diagnostics::Diagnostic;
    use image::{
        FinalImage, FinalImageLayout, FinalImageSection, FinalImageSymbolHandle,
        final_image_symbol_address,
    };
    use object_file::RelocationKind;

    const REBASE_TYPE_POINTER: u8 = 1;
    const REBASE_OPCODE_DONE: u8 = 0x00;
    const REBASE_OPCODE_SET_TYPE_IMM: u8 = 0x10;
    const REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB: u8 = 0x20;
    const REBASE_OPCODE_DO_REBASE_IMM_TIMES: u8 = 0x50;
    const POINTER_WIDTH: usize = 8;

    pub(crate) struct MachoRebaseInfo {
        pub(crate) bytes: Vec<u8>,
        sites: Vec<MachoRebaseSite>,
    }

    struct MachoRebaseSite {
        offset: usize,
        symbol: FinalImageSymbolHandle,
        addend: i64,
    }

    impl MachoRebaseInfo {
        pub(crate) fn validate_patched_preferred_pointers(
            &self,
            image: &FinalImage,
            layout: &FinalImageLayout,
        ) -> Result<Vec<crate::MachoRebasePointer>, Diagnostic> {
            let mut pointers = Vec::with_capacity(self.sites.len());
            for site in &self.sites {
                let symbol_address = final_image_symbol_address(image, site.symbol, layout)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "Mach-O rebase site lost its exact internal symbol before publication",
                        )
                    })?;
                let expected = symbol_address
                    .checked_add_signed(site.addend)
                    .ok_or_else(|| {
                        Diagnostic::error("Mach-O rebase preferred pointer overflows after addend")
                    })?;
                let end = site
                    .offset
                    .checked_add(POINTER_WIDTH)
                    .ok_or_else(|| Diagnostic::error("Mach-O rebase pointer site overflows"))?;
                let bytes = image.memory.data.get(site.offset..end).ok_or_else(|| {
                    Diagnostic::error("Mach-O rebase pointer site exceeds initialized data")
                })?;
                let actual = u64::from_le_bytes(bytes.try_into().map_err(|_| {
                    Diagnostic::error("Mach-O rebase pointer site has the wrong byte width")
                })?);
                if actual != expected {
                    return Err(Diagnostic::error(format!(
                        "Mach-O rebase pointer at data byte {} does not match its preferred internal symbol address",
                        site.offset
                    )));
                }
                pointers.push(crate::MachoRebasePointer {
                    data_offset: site.offset,
                    preferred_address: expected,
                });
            }
            Ok(pointers)
        }
    }

    pub(crate) fn macho_rebase_info_for_segment(
        image: &FinalImage,
        data_segment: u8,
    ) -> Result<MachoRebaseInfo, Diagnostic> {
        if data_segment > 0xf {
            return Err(Diagnostic::error(
                "Mach-O rebase segment index exceeds its four-bit immediate",
            ));
        }

        let mut sites = Vec::new();
        for (_, relocation) in image.relocation_table.relocations.iter() {
            if relocation.kind != RelocationKind::Absolute64 {
                continue;
            }
            if relocation.section != FinalImageSection::Data {
                return Err(Diagnostic::error(format!(
                    "Mach-O internal pointer relocation targets unsupported section {:?}; expected initialized data",
                    relocation.section
                )));
            }
            if relocation.byte_width != POINTER_WIDTH {
                return Err(Diagnostic::error(format!(
                    "Mach-O internal pointer relocation at data byte {} has width {}, expected {POINTER_WIDTH}",
                    relocation.offset, relocation.byte_width
                )));
            }
            if !relocation.offset.is_multiple_of(POINTER_WIDTH) {
                return Err(Diagnostic::error(format!(
                    "Mach-O internal pointer relocation at data byte {} is not pointer-aligned",
                    relocation.offset
                )));
            }
            let end = relocation
                .offset
                .checked_add(POINTER_WIDTH)
                .ok_or_else(|| Diagnostic::error("Mach-O internal pointer relocation overflows"))?;
            if end > image.memory.data.len() {
                return Err(Diagnostic::error(format!(
                    "Mach-O internal pointer relocation at data byte {} exceeds initialized data",
                    relocation.offset
                )));
            }
            if !image
                .symbol_table
                .symbols
                .is_valid(relocation.symbol_handle)
                || image
                    .symbol_table
                    .symbols
                    .get(relocation.symbol_handle)
                    .section
                    == FinalImageSection::None
            {
                return Err(Diagnostic::error(format!(
                    "Mach-O internal pointer relocation at data byte {} does not target an internal image symbol",
                    relocation.offset
                )));
            }
            sites.push(MachoRebaseSite {
                offset: relocation.offset,
                symbol: relocation.symbol_handle,
                addend: relocation.addend,
            });
        }
        sites.sort_unstable_by_key(|site| site.offset);
        if sites
            .windows(2)
            .any(|pair| pair[0].offset == pair[1].offset)
        {
            return Err(Diagnostic::error(
                "Mach-O internal pointer relocations contain a duplicate data site",
            ));
        }
        if sites.is_empty() {
            return Ok(MachoRebaseInfo {
                bytes: Vec::new(),
                sites,
            });
        }

        let mut bytes = vec![REBASE_OPCODE_SET_TYPE_IMM | REBASE_TYPE_POINTER];
        for site in &sites {
            bytes.push(REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB | data_segment);
            write_uleb128(&mut bytes, site.offset as u64);
            bytes.push(REBASE_OPCODE_DO_REBASE_IMM_TIMES | 1);
        }
        bytes.push(REBASE_OPCODE_DONE);
        Ok(MachoRebaseInfo { bytes, sites })
    }

    #[cfg(test)]
    mod tests {
        use super::macho_rebase_info_for_segment;
        const EXECUTABLE_DATA_SEGMENT: u8 = 2;
        use arena::Handle;
        use image::{
            FinalImage, FinalImageLayout, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
        };

        use crate::test_support::relocation_mut;
        use object_file::{RelocationKind, SymbolKind};

        fn private_function(
            image: &mut FinalImage,
            offset: usize,
            size: usize,
        ) -> Handle<FinalImageSymbol> {
            image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "private-realization".to_owned(),
                section: FinalImageSection::Text,
                offset,
                size,
                kind: SymbolKind::Function,
            })
        }

        fn data_pointer_relocation(
            image: &mut FinalImage,
            offset: usize,
            addend: i64,
            symbol: Handle<FinalImageSymbol>,
        ) -> Handle<FinalImageRelocation> {
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Data,
                    offset,
                    byte_width: 8,
                    symbol_handle: symbol,
                    addend,
                    kind: RelocationKind::Absolute64,
                })
        }

        #[test]
        fn data_absolute_relocations_become_exact_dyld_pointer_sites() {
            let mut image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    data: vec![0; 0x98],
                    ..Default::default()
                },
            );
            let function = private_function(&mut image, 4, 4);
            for offset in [0x90, 0x08] {
                data_pointer_relocation(&mut image, offset, 0, function);
            }

            assert_eq!(
                macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT)
                    .expect("valid rebase sites")
                    .bytes,
                vec![0x11, 0x22, 0x08, 0x51, 0x22, 0x90, 0x01, 0x51, 0x00]
            );
        }

        #[test]
        fn rejects_non_data_or_malformed_absolute_relocations() {
            let mut image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    text: vec![0; 8],
                    data: vec![0; 8],
                    ..Default::default()
                },
            );
            let function = private_function(&mut image, 0, 8);
            let relocation = image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: 0,
                    byte_width: 8,
                    symbol_handle: function,
                    addend: 0,
                    kind: RelocationKind::Absolute64,
                });
            assert!(macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT).is_err());

            relocation_mut(&mut image, relocation).section = FinalImageSection::Data;
            relocation_mut(&mut image, relocation).byte_width = 4;
            assert!(macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT).is_err());

            relocation_mut(&mut image, relocation).byte_width = 8;
            relocation_mut(&mut image, relocation).offset = 1;
            assert!(macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT).is_err());
        }

        #[test]
        fn preferred_pointer_replay_rejects_mutated_patched_bytes() {
            let mut image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    text: vec![0; 8],
                    data: vec![0; 8],
                    ..Default::default()
                },
            );
            let function = private_function(&mut image, 4, 4);
            data_pointer_relocation(&mut image, 0, 3, function);
            let info = macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT)
                .expect("valid internal pointer");
            let layout = FinalImageLayout {
                text_address: 0x1_0000_1000,
                data_address: 0x1_0000_4000,
                bss_address: 0x1_0000_5000,
            };
            image::apply_relocations(&mut image, &layout, "test Mach-O")
                .expect("preferred pointer patch");
            info.validate_patched_preferred_pointers(&image, &layout)
                .expect("exact patched pointer");

            image.memory.data[0] ^= 1;
            assert!(
                info.validate_patched_preferred_pointers(&image, &layout)
                    .is_err()
            );
        }

        #[test]
        fn rejects_unknown_targets_and_duplicate_pointer_sites() {
            let mut image = crate::test_support::image(
                FinalImage::default().target,
                image::FinalImageMemory {
                    data: vec![0; 8],
                    ..Default::default()
                },
            );
            let function = private_function(&mut image, 0, 4);
            let first = data_pointer_relocation(&mut image, 0, 0, Handle::invalid());
            assert!(macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT).is_err());

            relocation_mut(&mut image, first).symbol_handle = function;
            data_pointer_relocation(&mut image, 0, 0, function);
            assert!(macho_rebase_info_for_segment(&image, EXECUTABLE_DATA_SEGMENT).is_err());
        }
    }
}

pub(crate) fn write_uleb128(bytes: &mut Vec<u8>, mut value: u64) {
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
