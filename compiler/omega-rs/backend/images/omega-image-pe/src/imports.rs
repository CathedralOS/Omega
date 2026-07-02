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
        .symbol_table
        .imports
        .iter()
        .filter_map(|(_, import)| {
            image
                .symbol_table
                .symbols
                .is_valid(import.symbol_handle)
                .then_some(import.symbol_handle)
        })
        .collect::<Vec<_>>();
    let mut thunks = Vec::new();

    for symbol_handle in imports {
        let symbol = image.symbol_table.symbols.get(symbol_handle).name.clone();
        let text_offset = image.memory.text.len();
        image.memory.text.extend([0xff, 0x25, 0, 0, 0, 0]);

        let image_symbol = image.symbol_table.symbols.get_mut(symbol_handle);
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
    // MULTI-DLL import table: thunks are grouped by their catalog library
    // (default KERNEL32.dll -- the historical single-DLL behavior for symbols
    // outside the Windows catalog). Layout, all offsets relative to rdata_rva:
    //   [descriptors: (n_dlls + 1) * 20]
    //   [ILT_dll1][ILT_dll2]...      (each (count+1) * 8)
    //   [IAT_dll1][IAT_dll2]...      (contiguous, so ONE directory entry covers all)
    //   [dll names][hint/name entries]
    // `iat_rvas` is returned in the INPUT thunk order (patch_import_thunks zips
    // it against the same slice).
    let mut libraries: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (index, import) in imports.iter().enumerate() {
        let library = omega_calling_conventions::windows_import_library(&import.symbol)
            .unwrap_or("KERNEL32.dll");
        match libraries.iter_mut().find(|(name, _)| *name == library) {
            Some((_, members)) => members.push(index),
            None => libraries.push((library, vec![index])),
        }
    }

    let descriptor_table_size = (libraries.len() + 1) * 20;
    // Per-library ILT offsets, then the contiguous IAT region.
    let mut ilt_offsets = Vec::with_capacity(libraries.len());
    let mut cursor = descriptor_table_size;
    for (_, members) in &libraries {
        ilt_offsets.push(cursor);
        cursor += (members.len() + 1) * 8;
    }
    let iat_region_offset = cursor;
    let mut iat_offsets = Vec::with_capacity(libraries.len());
    for (_, members) in &libraries {
        iat_offsets.push(cursor);
        cursor += (members.len() + 1) * 8;
    }
    let iat_region_size = cursor - iat_region_offset;
    // DLL name strings.
    let mut dll_name_offsets = Vec::with_capacity(libraries.len());
    for (library, _) in &libraries {
        dll_name_offsets.push(cursor);
        cursor += library.len() + 1;
    }
    cursor = align_to(cursor, 2);
    // Hint/name entries, in INPUT thunk order.
    let mut hint_name_offsets = vec![0usize; imports.len()];
    for (index, import) in imports.iter().enumerate() {
        hint_name_offsets[index] = cursor;
        cursor = align_to(cursor + 2 + import.symbol.len() + 1, 2);
    }

    let mut bytes = vec![0; cursor];

    for (library_index, (library, members)) in libraries.iter().enumerate() {
        let descriptor_offset = library_index * 20;
        let ilt_rva = rdata_rva + ilt_offsets[library_index] as u32;
        let iat_rva = rdata_rva + iat_offsets[library_index] as u32;
        let dll_name_rva = rdata_rva + dll_name_offsets[library_index] as u32;
        write_u32_at(&mut bytes, descriptor_offset, ilt_rva);
        write_u32_at(&mut bytes, descriptor_offset + 12, dll_name_rva);
        write_u32_at(&mut bytes, descriptor_offset + 16, iat_rva);

        let name_offset = dll_name_offsets[library_index];
        bytes[name_offset..name_offset + library.len()].copy_from_slice(library.as_bytes());

        for (slot, import_index) in members.iter().enumerate() {
            let hint_name_rva = rdata_rva + hint_name_offsets[*import_index] as u32;
            write_u64_at(
                &mut bytes,
                ilt_offsets[library_index] + slot * 8,
                u64::from(hint_name_rva),
            );
            write_u64_at(
                &mut bytes,
                iat_offsets[library_index] + slot * 8,
                u64::from(hint_name_rva),
            );
        }
    }

    // Hint/name entries + per-thunk IAT rvas in INPUT order.
    let mut iat_rvas = vec![0u32; imports.len()];
    for (library_index, (_, members)) in libraries.iter().enumerate() {
        for (slot, import_index) in members.iter().enumerate() {
            iat_rvas[*import_index] =
                rdata_rva + (iat_offsets[library_index] + slot * 8) as u32;
        }
    }
    for (index, import) in imports.iter().enumerate() {
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
        iat_rva: rdata_rva + iat_region_offset as u32,
        iat_size: iat_region_size,
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
        write_i32_at(&mut image.memory.text, thunk.text_offset + 2, displacement)?;
    }

    Ok(())
}
