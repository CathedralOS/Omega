mod aarch64_relocations;
mod model;

pub use aarch64_relocations::apply_aarch64_relocations;
pub use model::{
    FinalImage, FinalImageImport, FinalImageLayout, FinalImageRelocation, FinalImageSection,
    FinalImageSymbol, FinalImageSymbolHandle,
};
use omega_core::arena::{Arena, Handle};
use omega_object::{
    ObjectPlan, ObjectSymbolHandle, RelocationPlan, SectionKind, SymbolKind, SymbolSection,
};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImageOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub imports: usize,
    pub relocations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedImageOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub kind: ImageOutputKind,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
    pub final_image_symbols: usize,
    pub final_image_imports: usize,
    pub final_image_relocations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOutputKind {
    DirectExecutable,
}

pub fn emitted_direct_executable_output(output: ExecutableImageOutput) -> EmittedImageOutput {
    EmittedImageOutput {
        bytes: output.bytes,
        file_name: output.file_name,
        format: output.format,
        kind: ImageOutputKind::DirectExecutable,
        text_bytes: output.text_bytes,
        data_bytes: output.data_bytes,
        bss_bytes: output.bss_bytes,
        symbols: output.symbols,
        relocations: output.relocations,
        final_image_symbols: output.symbols,
        final_image_imports: output.imports,
        final_image_relocations: output.relocations,
    }
}

pub struct FinalImageInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

pub fn build_final_image(input: FinalImageInput<'_>) -> FinalImage {
    let import_count = input
        .object
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
        .count();
    let mut image = FinalImage {
        target: input.target,
        entry_symbol: final_image_symbol_handle(input.object.entry_symbol),
        text: input.text_bytes.to_vec(),
        data: input.data_bytes.to_vec(),
        bss_size: section_size(input.object, SectionKind::Bss),
        bss_alignment: section_alignment(input.object, SectionKind::Bss),
        symbols: Arena::with_capacity(input.object.symbols.len()),
        imports: Arena::with_capacity(import_count),
        relocations: Arena::with_capacity(input.relocations.records.len()),
    };

    image.symbols.insert_many(
        input
            .object
            .symbols
            .iter()
            .map(|(_, symbol)| FinalImageSymbol {
                name: symbol.name.clone(),
                section: final_image_section(symbol.section),
                offset: symbol.offset,
                size: symbol.size,
                kind: symbol.kind,
            }),
    );

    image.imports.insert_many(
        input
            .object
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
            .map(|(symbol_handle, _)| FinalImageImport {
                symbol_handle: final_image_symbol_handle(symbol_handle),
            }),
    );

    let symbols = &image.symbols;
    image
        .relocations
        .insert_many(input.relocations.records.iter().map(|(_, relocation)| {
            let symbol_handle = final_image_symbol_handle(relocation.symbol_handle);
            FinalImageRelocation {
                text_offset: relocation.text_offset,
                byte_width: relocation.byte_width,
                symbol_handle: symbol_handle
                    .is_valid()
                    .then_some(symbol_handle)
                    .filter(|handle| symbols.is_valid(*handle))
                    .unwrap_or_else(Handle::invalid),
                kind: relocation.kind,
            }
        }));

    image
}

fn final_image_symbol_handle(symbol: ObjectSymbolHandle) -> FinalImageSymbolHandle {
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
    if !image.symbols.is_valid(symbol) {
        return None;
    }

    let symbol = image.symbols.get(symbol);
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
        .imports
        .iter()
        .any(|(_, import)| import.symbol_handle == symbol)
}

pub fn final_image_symbol_name(image: &FinalImage, symbol: FinalImageSymbolHandle) -> &str {
    if image.symbols.is_valid(symbol) {
        image.symbols.get(symbol).name.as_str()
    } else {
        ""
    }
}

fn final_image_section(section: SymbolSection) -> FinalImageSection {
    match section {
        SymbolSection::None => FinalImageSection::None,
        SymbolSection::Section(SectionKind::Text) => FinalImageSection::Text,
        SymbolSection::Section(SectionKind::Data) => FinalImageSection::Data,
        SymbolSection::Section(SectionKind::Bss) => FinalImageSection::Bss,
    }
}

fn section_size(object: &ObjectPlan, kind: SectionKind) -> usize {
    object
        .sections
        .iter()
        .find(|(_, section)| section.kind == kind)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn section_alignment(object: &ObjectPlan, kind: SectionKind) -> usize {
    object
        .sections
        .iter()
        .find(|(_, section)| section.kind == kind)
        .map(|(_, section)| section.alignment)
        .unwrap_or(1)
}
