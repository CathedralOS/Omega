use crate::layout::align_to;
use omega_core::diagnostics::Diagnostic;
use omega_image::{FinalImage, FinalImageLayout, FinalImageSection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachoImportThunk {
    pub(crate) symbol: String,
    pub(crate) text_offset: usize,
    pub(crate) data_offset: usize,
}

pub(crate) fn install_import_thunks(image: &mut FinalImage) -> Vec<MachoImportThunk> {
    let imports = image
        .imports
        .iter()
        .filter_map(|(_, import)| {
            image
                .symbols
                .is_valid(import.symbol_handle)
                .then_some(import.symbol_handle)
        })
        .collect::<Vec<_>>();
    let mut thunks = Vec::new();

    for symbol_handle in imports {
        let symbol = image.symbols.get(symbol_handle).name.clone();
        let text_offset = image.text.len();
        image.data.resize(align_to(image.data.len(), 8), 0);
        let data_offset = image.data.len();
        image.text.extend([0u8; 12]);
        image.data.extend([0u8; 8]);

        let image_symbol = image.symbols.get_mut(symbol_handle);
        image_symbol.section = FinalImageSection::Text;
        image_symbol.offset = text_offset;
        image_symbol.size = 12;

        thunks.push(MachoImportThunk {
            symbol,
            text_offset,
            data_offset,
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
            &mut image.text,
            thunk.text_offset,
            instruction_address,
            pointer_address,
        )?;
        patch_aarch64_ldr_x_from_page(
            &mut image.text,
            thunk.text_offset + 4,
            pointer_address,
            16,
            16,
        )?;
        write_u32_at(&mut image.text, thunk.text_offset + 8, 0xd61f_0200)?;
    }

    Ok(())
}

pub(crate) fn macho_bind_info(thunks: &[MachoImportThunk]) -> Vec<u8> {
    let mut bytes = Vec::new();

    for thunk in thunks {
        bytes.push(0x11);
        bytes.push(0x40);
        bytes.extend(thunk.symbol.as_bytes());
        bytes.push(0);
        bytes.push(0x51);
        bytes.push(0x72);
        write_uleb128(&mut bytes, thunk.data_offset as u64);
        bytes.push(0x90);
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
