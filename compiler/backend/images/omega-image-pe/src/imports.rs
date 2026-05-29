use crate::bytes::{write_i32_at, write_u16_at, write_u32_at, write_u64_at};
use crate::constants::IMAGE_BASE;
use crate::layout::align_to;
use omega_core::diagnostics::Diagnostic;
use omega_image::{FinalImage, FinalImageLayout, FinalImageSection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportThunk {
    pub(crate) symbol: String,
    pub(crate) text_offset: usize,
}

pub(crate) fn install_import_thunks(image: &mut FinalImage) -> Vec<PeImportThunk> {
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
        image.text.extend([0xff, 0x25, 0, 0, 0, 0]);

        let image_symbol = image.symbols.get_mut(symbol_handle);
        image_symbol.section = FinalImageSection::Text;
        image_symbol.offset = text_offset;
        image_symbol.size = 6;

        thunks.push(PeImportThunk {
            symbol,
            text_offset,
        });
    }

    thunks
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportTable {
    pub(crate) bytes: Vec<u8>,
    pub(crate) iat_rvas: Vec<u32>,
    pub(crate) import_directory_rva: u32,
    pub(crate) import_directory_size: usize,
    pub(crate) iat_rva: u32,
    pub(crate) iat_size: usize,
}

pub(crate) fn build_import_table(imports: &[PeImportThunk], rdata_rva: u32) -> PeImportTable {
    let descriptor_offset = 0usize;
    let descriptor_count = usize::from(!imports.is_empty());
    let descriptor_table_size = (descriptor_count + 1) * 20;
    let ilt_offset = descriptor_table_size;
    let thunk_table_size = (imports.len() + 1) * 8;
    let iat_offset = ilt_offset + thunk_table_size;
    let dll_name_offset = iat_offset + thunk_table_size;
    let mut name_cursor = align_to(dll_name_offset + b"KERNEL32.dll\0".len(), 2);
    let mut hint_name_offsets = Vec::with_capacity(imports.len());

    for import in imports {
        hint_name_offsets.push(name_cursor);
        name_cursor = align_to(name_cursor + 2 + import.symbol.len() + 1, 2);
    }

    let mut bytes = vec![0; name_cursor];
    let ilt_rva = rdata_rva + ilt_offset as u32;
    let iat_rva = rdata_rva + iat_offset as u32;
    let dll_name_rva = rdata_rva + dll_name_offset as u32;

    if !imports.is_empty() {
        write_u32_at(&mut bytes, descriptor_offset, ilt_rva);
        write_u32_at(&mut bytes, descriptor_offset + 12, dll_name_rva);
        write_u32_at(&mut bytes, descriptor_offset + 16, iat_rva);
    }

    bytes[dll_name_offset..dll_name_offset + b"KERNEL32.dll\0".len()]
        .copy_from_slice(b"KERNEL32.dll\0");

    let mut iat_rvas = Vec::with_capacity(imports.len());
    for (index, import) in imports.iter().enumerate() {
        let hint_name_rva = rdata_rva + hint_name_offsets[index] as u32;
        write_u64_at(&mut bytes, ilt_offset + index * 8, u64::from(hint_name_rva));
        write_u64_at(&mut bytes, iat_offset + index * 8, u64::from(hint_name_rva));
        iat_rvas.push(iat_rva + (index * 8) as u32);

        let name_offset = hint_name_offsets[index];
        write_u16_at(&mut bytes, name_offset, 0);
        let symbol_start = name_offset + 2;
        bytes[symbol_start..symbol_start + import.symbol.len()]
            .copy_from_slice(import.symbol.as_bytes());
    }

    PeImportTable {
        bytes,
        iat_rvas,
        import_directory_rva: rdata_rva,
        import_directory_size: descriptor_table_size,
        iat_rva,
        iat_size: thunk_table_size,
    }
}

pub(crate) fn patch_import_thunks(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    thunks: &[PeImportThunk],
    iat_rvas: &[u32],
) -> Result<(), Diagnostic> {
    for (thunk, iat_rva) in thunks.iter().zip(iat_rvas.iter().copied()) {
        let instruction_address = layout.text_address + thunk.text_offset as u64;
        let next_instruction = instruction_address + 6;
        let iat_address = IMAGE_BASE + u64::from(iat_rva);
        let delta = iat_address as i64 - next_instruction as i64;
        let displacement = i32::try_from(delta).map_err(|_| {
            Diagnostic::error(format!(
                "PE x86_64 import thunk for `{}` is out of range",
                thunk.symbol
            ))
        })?;
        write_i32_at(&mut image.text, thunk.text_offset + 2, displacement)?;
    }

    Ok(())
}
