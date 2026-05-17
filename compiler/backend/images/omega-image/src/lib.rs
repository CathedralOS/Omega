mod aarch64_relocations;
mod model;

pub use aarch64_relocations::apply_aarch64_relocations;
pub use model::{
    FinalImage, FinalImageImport, FinalImageLayout, FinalImageRelocation, FinalImageSection,
    FinalImageSymbol, FinalImageSymbolHandle,
};
use omega_core::arena::{Arena, Handle};
use omega_object::{ObjectPlan, ObjectSymbolHandle, RelocationPlan, SectionKind, SymbolKind};
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
        entry_symbol: input.object.entry_symbol.clone(),
        text: input.text_bytes.to_vec(),
        data: input.data_bytes.to_vec(),
        bss_size: section_size(input.object, SectionKind::Bss),
        bss_alignment: section_alignment(input.object, SectionKind::Bss),
        symbols: Arena::with_capacity(input.object.symbols.len()),
        imports: Arena::with_capacity(import_count),
        relocations: Arena::with_capacity(input.relocations.records.len()),
    };

    image
        .symbols
        .insert_many(input.object.symbols.iter().map(|(_, symbol)| {
            FinalImageSymbol {
                name: symbol.name.clone(),
                section: symbol
                    .section
                    .as_deref()
                    .map(final_image_section)
                    .unwrap_or(FinalImageSection::None),
                offset: symbol.offset,
                size: symbol.size,
                kind: symbol.kind,
            }
        }));

    image.imports.insert_many(
        input
            .object
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
            .map(|(_, symbol)| FinalImageImport {
                symbol: symbol.name.clone(),
            }),
    );

    let symbols = &image.symbols;
    image
        .relocations
        .insert_many(input.relocations.records.iter().map(|(_, relocation)| {
            FinalImageRelocation {
                text_offset: relocation.text_offset,
                byte_width: relocation.byte_width,
                symbol: relocation.symbol.clone(),
                symbol_handle: final_image_symbol_handle(relocation.symbol_handle)
                    .is_valid()
                    .then(|| final_image_symbol_handle(relocation.symbol_handle))
                    .filter(|handle| symbols.is_valid(*handle))
                    .unwrap_or_else(|| symbol_handle(symbols, &relocation.symbol)),
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

pub fn final_image_imports_symbol(image: &FinalImage, symbol_name: &str) -> bool {
    image
        .imports
        .iter()
        .any(|(_, import)| import.symbol == symbol_name)
}

fn symbol_handle(symbols: &Arena<FinalImageSymbol>, symbol_name: &str) -> FinalImageSymbolHandle {
    symbols
        .iter()
        .find(|(_, symbol)| symbol.name == symbol_name)
        .map(|(handle, _)| handle)
        .unwrap_or_else(Handle::invalid)
}

fn final_image_section(section_name: &str) -> FinalImageSection {
    match section_name {
        ".text" | "__TEXT,__text" => FinalImageSection::Text,
        ".data" | "__DATA,__data" => FinalImageSection::Data,
        ".bss" | "__DATA,__bss" => FinalImageSection::Bss,
        _ => FinalImageSection::None,
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
