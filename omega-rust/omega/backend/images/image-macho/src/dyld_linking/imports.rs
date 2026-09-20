//! The dylib roster, the jump thunks, the eager bind stream, and the post-relocation
//! re-check that the thunk opcodes survived.

use crate::dyld_linking::load_commands::MachoDylib;
use crate::file_layout::layout::align_to;
use crate::isa::MachoIsa;
use calling_conventions::{DARWIN_LIBOBJC_PATH, darwin_import_library};
use diagnostics::Diagnostic;
use image::{
    FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
    FinalImage, FinalImageImportPlan, FinalImageLayout, FinalImageSection, FinalImageSymbolHandle,
    PlacedDataRegionInventory, PlacedExecutableRegionInventory,
};
use object_file::SymbolKind;
use target::{ForeignLocatorCandidate, NormalizedForeignLocator};

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
    isa: MachoIsa,
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
                if locator.target() != isa.target_profile()
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
                // `jmp qword ptr [rip + disp32]`: the slot lives in __DATA, so
                // the displacement reaches across the planned segment distance.
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
                slot[2..6].copy_from_slice(&displacement.to_le_bytes());
            }
        }
    }

    Ok(())
}

/// Validate the final patched Mach-O thunk opcode shape and attach its exact
/// architectural effect. The AArch64 sequence uses X16 as its sole scratch
/// register; the x86-64 stub writes the instruction pointer alone.
pub(crate) fn validate_import_thunk_footprints(
    image: &mut FinalImage,
    thunks: &[MachoImportThunk],
    isa: MachoIsa,
) -> Result<(), Diagnostic> {
    let thunk_size = isa.import_thunk_size();
    for thunk in thunks {
        let end = thunk.text_offset.checked_add(thunk_size).ok_or_else(|| {
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
            return Err(Diagnostic::error(format!(
                "Mach-O import thunk `{}` does not match the ISA's canonical binding-slot branch",
                thunk.symbol
            )));
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
                Diagnostic::error(format!(
                    "Mach-O import thunk `{}` is missing its executable-region record",
                    thunk.symbol
                ))
            })?;
        region.footprint = Some(isa.thunk_footprint());
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

/// Replay the exact thunk↔binding-slot pairing for an AArch64 image.
pub fn validate_macho_aarch64_import_binding_pairing(
    final_text_bytes: &[u8],
    executable_regions: &PlacedExecutableRegionInventory,
    data_regions: &PlacedDataRegionInventory,
) -> Result<(), Diagnostic> {
    validate_macho_import_binding_pairing(
        final_text_bytes,
        executable_regions,
        data_regions,
        MachoIsa::Aarch64,
    )
}

/// Replay the exact thunk↔binding-slot pairing for an x86-64 image.
pub fn validate_macho_x86_64_import_binding_pairing(
    final_text_bytes: &[u8],
    executable_regions: &PlacedExecutableRegionInventory,
    data_regions: &PlacedDataRegionInventory,
) -> Result<(), Diagnostic> {
    validate_macho_import_binding_pairing(
        final_text_bytes,
        executable_regions,
        data_regions,
        MachoIsa::X86_64,
    )
}

/// Replay the exact thunk↔binding-slot pairing against the placed region
/// inventories. Every import thunk must decode to its ISA's canonical
/// branch-to-slot sequence that loads its binding slot, and each exercised
/// thunk must pair with exactly one placed binding slot naming the same
/// symbol — and vice versa. A resolver-returned address or an unrecorded slot
/// cannot substitute for this placed custody.
fn validate_macho_import_binding_pairing(
    final_text_bytes: &[u8],
    executable_regions: &PlacedExecutableRegionInventory,
    data_regions: &PlacedDataRegionInventory,
    isa: MachoIsa,
) -> Result<(), Diagnostic> {
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

/// Decode the thunk's canonical branch-to-slot sequence for `isa` and return
/// the initialized-data pointer address it loads.
fn thunk_bound_pointer_address(
    final_text_bytes: &[u8],
    region: &image::PlacedExecutableRegion,
    isa: MachoIsa,
) -> Result<u64, Diagnostic> {
    let thunk_size = isa.import_thunk_size();
    let end = region
        .section_offset
        .checked_add(thunk_size)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "Mach-O import thunk `{}` range overflows",
                region.symbol
            ))
        })?;
    let bytes = final_text_bytes
        .get(region.section_offset..end)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "Mach-O import thunk `{}` is out of final .text bounds",
                region.symbol
            ))
        })?;
    match isa {
        MachoIsa::Aarch64 => {
            let adrp = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte ADRP"));
            let ldr = u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte LDR"));
            let br = u32::from_le_bytes(bytes[8..12].try_into().expect("four-byte BR"));
            if adrp & 0x9f00_001f != 0x9000_0010
                || ldr & 0xffc0_03ff != 0xf940_0210
                || br != 0xd61f_0200
            {
                return Err(Diagnostic::error(format!(
                    "Mach-O import thunk `{}` does not match ADRP X16; LDR X16, [X16, #imm]; BR X16",
                    region.symbol
                )));
            }
            let immediate = i64::from((((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0b11));
            // Sign-extend the 21-bit page delta.
            let page_delta = (immediate << 43) >> 43;
            let slot_page = (region.address & !0xfff)
                .checked_add_signed(page_delta.checked_mul(4096).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Mach-O import thunk `{}` page delta overflows",
                        region.symbol
                    ))
                })?)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Mach-O import thunk `{}` bound page overflows",
                        region.symbol
                    ))
                })?;
            let scaled_offset = u64::from((ldr >> 10) & 0xfff);
            slot_page.checked_add(scaled_offset * 8).ok_or_else(|| {
                Diagnostic::error(format!(
                    "Mach-O import thunk `{}` bound pointer address overflows",
                    region.symbol
                ))
            })
        }
        MachoIsa::X86_64 => {
            let displacement = isa_x86_64::decode_x86_64_import_thunk(bytes).ok_or_else(|| {
                Diagnostic::error(format!(
                    "Mach-O import thunk `{}` does not match JMP qword ptr [rip + disp32]",
                    region.symbol
                ))
            })?;
            // The displacement is measured from the byte after the six-byte stub.
            (region.address + 6)
                .checked_add_signed(i64::from(displacement))
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Mach-O import thunk `{}` bound pointer address overflows",
                        region.symbol
                    ))
                })
        }
    }
}

#[cfg(test)]
mod tests;
