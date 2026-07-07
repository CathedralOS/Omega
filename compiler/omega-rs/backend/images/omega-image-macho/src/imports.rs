use crate::layout::align_to;
use crate::load_commands::MachoDylib;
use omega_calling_conventions::{DARWIN_LIBOBJC_PATH, darwin_import_library};
use omega_core::diagnostics::Diagnostic;
use omega_image::{FinalImage, FinalImageLayout, FinalImageSection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachoImportThunk {
    pub(crate) symbol: String,
    pub(crate) text_offset: usize,
    pub(crate) data_offset: usize,
    /// The install name of the dylib this symbol binds against — selects the
    /// bind-info dylib ordinal (`macho_dylib_list` position + 1).
    pub(crate) library: &'static str,
}

/// The ordered, de-duplicated dylibs this image links against. libSystem is ALWAYS
/// first (ordinal 1) — dyld requires the C runtime and every program links it —
/// then any others (e.g. libobjc) in first-appearance order. A thunk's dylib
/// ordinal is its library's index here, plus 1.
pub(crate) fn macho_dylib_list(thunks: &[MachoImportThunk]) -> Vec<MachoDylib> {
    let mut dylibs = vec![MachoDylib::LIBSYSTEM];
    let mut uses_objc = false;
    for thunk in thunks {
        if thunk.library == DARWIN_LIBOBJC_PATH {
            uses_objc = true;
        }
        // Every imported symbol's library must carry a load command (CoreGraphics
        // `_CG*` calls, libobjc, …). libSystem is already present; an unknown path
        // is a wiring bug (a symbol mapped to a dylib with no spec) — skipped so no
        // malformed load command is emitted.
        ensure_dylib(&mut dylibs, thunk.library);
    }
    // A program that touches the Objective-C runtime needs the Cocoa frameworks
    // LOADED so their classes REGISTER — `objc_getClass("NSString"/"NSWindow")`
    // only sees classes from loaded dylibs (libobjc alone provides just `NSObject`
    // + the runtime). These carry NO imported symbols; they are loaded purely for
    // their class-registration side effect (appended AFTER libobjc so libobjc
    // keeps ordinal 2 and existing binds are unaffected). See D-objc-load.
    // `ensure_dylib` de-dups, so a directly-called CoreGraphics is not doubled.
    if uses_objc {
        ensure_dylib(&mut dylibs, MachoDylib::FOUNDATION.path);
        ensure_dylib(&mut dylibs, MachoDylib::APPKIT.path);
        ensure_dylib(&mut dylibs, MachoDylib::COREGRAPHICS.path);
    }
    dylibs
}

/// Append the `MachoDylib` spec for `path` if it is not already loaded. An unknown
/// path (no matching spec) is skipped — a symbol mapped to a dylib with no spec is
/// a wiring bug, and emitting a malformed load command would be worse.
fn ensure_dylib(dylibs: &mut Vec<MachoDylib>, path: &str) {
    if dylibs.iter().any(|dylib| dylib.path == path) {
        return;
    }
    if let Some(spec) = dylib_spec_for(path) {
        dylibs.push(spec);
    }
}

/// The `MachoDylib` spec (path + versions) for a known install-name path.
fn dylib_spec_for(path: &str) -> Option<MachoDylib> {
    [
        MachoDylib::LIBSYSTEM,
        MachoDylib::LIBOBJC,
        MachoDylib::FOUNDATION,
        MachoDylib::APPKIT,
        MachoDylib::COREGRAPHICS,
    ]
    .into_iter()
    .find(|spec| spec.path == path)
}

fn dylib_ordinal(dylibs: &[MachoDylib], library: &str) -> u8 {
    dylibs
        .iter()
        .position(|dylib| dylib.path == library)
        .map(|index| index as u8 + 1)
        .unwrap_or(1)
}

pub(crate) fn install_import_thunks(image: &mut FinalImage) -> Vec<MachoImportThunk> {
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
    let imports = image
        .symbol_table
        .imports
        .iter()
        .filter_map(|(_, import)| {
            (image.symbol_table.symbols.is_valid(import.symbol_handle)
                && referenced.contains(&import.symbol_handle))
            .then_some(import.symbol_handle)
        })
        .collect::<Vec<_>>();
    let mut thunks = Vec::new();

    for symbol_handle in imports {
        let symbol = image.symbol_table.symbols.get(symbol_handle).name.clone();
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

        let library = darwin_import_library(&symbol);
        thunks.push(MachoImportThunk {
            symbol,
            text_offset,
            data_offset,
            library,
        });
    }

    thunks
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

pub(crate) fn macho_bind_info(thunks: &[MachoImportThunk], dylibs: &[MachoDylib]) -> Vec<u8> {
    let mut bytes = Vec::new();

    for thunk in thunks {
        // BIND_OPCODE_SET_DYLIB_ORDINAL_IMM | ordinal — which LC_LOAD_DYLIB this
        // symbol resolves through (1 = libSystem, 2 = libobjc, …). Ordinals ≤ 15
        // fit the immediate; the ordinal comes from `macho_dylib_list` order.
        let ordinal = dylib_ordinal(dylibs, thunk.library);
        debug_assert!(ordinal <= 0xf, "Mach-O dylib ordinal {ordinal} exceeds IMM");
        bytes.push(0x10 | ordinal);
        bytes.push(0x40); // SET_SYMBOL_TRAILING_FLAGS_IMM | 0
        bytes.extend(thunk.symbol.as_bytes());
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
