use crate::object::{SectionKind, SymbolKind};
use crate::plan::NativePlan;
use crate::relocations::RelocationKind;
use crate::target::NativeTarget;
use omega_core::arena::{Arena, Handle};
use omega_core::diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImage {
    pub target: NativeTarget,
    pub entry_symbol: String,
    pub text: Vec<u8>,
    pub data: Vec<u8>,
    pub bss_size: usize,
    pub bss_alignment: usize,
    pub symbols: Arena<FinalImageSymbol>,
    pub imports: Arena<FinalImageImport>,
    pub relocations: Arena<FinalImageRelocation>,
}

impl Default for FinalImage {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            entry_symbol: String::new(),
            text: Vec::new(),
            data: Vec::new(),
            bss_size: 0,
            bss_alignment: 1,
            symbols: Arena::new(),
            imports: Arena::new(),
            relocations: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageSymbol {
    pub name: String,
    pub section: FinalImageSection,
    pub offset: usize,
    pub size: usize,
    pub kind: SymbolKind,
}

pub type FinalImageSymbolHandle = Handle<FinalImageSymbol>;

impl Default for FinalImageSymbol {
    fn default() -> Self {
        Self {
            name: String::new(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Object,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinalImageSection {
    Text,
    Data,
    Bss,
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FinalImageImport {
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalImageRelocation {
    pub text_offset: usize,
    pub byte_width: usize,
    pub symbol: String,
    pub symbol_handle: FinalImageSymbolHandle,
    pub kind: RelocationKind,
}

impl Default for FinalImageRelocation {
    fn default() -> Self {
        Self {
            text_offset: 0,
            byte_width: 0,
            symbol: String::new(),
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Aarch64Branch26,
        }
    }
}

pub fn build_final_image(native_plan: &NativePlan) -> FinalImage {
    let mut image = FinalImage {
        target: native_plan.target,
        entry_symbol: native_plan.object.entry_symbol.clone(),
        text: native_plan.machine_code.bytes.storage_slice().to_vec(),
        data: native_plan.data.bytes.storage_slice().to_vec(),
        bss_size: section_size(native_plan, SectionKind::Bss),
        bss_alignment: section_alignment(native_plan, SectionKind::Bss),
        symbols: Arena::new(),
        imports: Arena::new(),
        relocations: Arena::new(),
    };

    image
        .symbols
        .insert_many(native_plan.object.symbols.iter().map(|(_, symbol)| {
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
        native_plan
            .object
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
            .map(|(_, symbol)| FinalImageImport {
                symbol: symbol.name.clone(),
            }),
    );

    let symbols = &image.symbols;
    image.relocations.insert_many(
        native_plan
            .relocations
            .records
            .iter()
            .map(|(_, relocation)| FinalImageRelocation {
                text_offset: relocation.text_offset,
                byte_width: relocation.byte_width,
                symbol: relocation.symbol.clone(),
                symbol_handle: symbol_handle(symbols, &relocation.symbol),
                kind: relocation.kind,
            }),
    );

    image
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

pub fn apply_aarch64_relocations(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    output_name: &str,
) -> Result<(), Diagnostic> {
    for (_, relocation) in image.relocations.iter() {
        let Some(symbol_address) =
            final_image_symbol_address(image, relocation.symbol_handle, layout)
        else {
            if final_image_imports_symbol(image, &relocation.symbol) {
                return Err(Diagnostic::error(format!(
                    "{output_name} cannot import `{}` yet; use syscalls or add dynamic binding",
                    relocation.symbol
                )));
            }

            return Err(Diagnostic::error(format!(
                "{output_name} relocation references unknown symbol `{}`",
                relocation.symbol
            )));
        };

        match relocation.kind {
            RelocationKind::Aarch64Page21 => {
                patch_aarch64_adrp(
                    &mut image.text,
                    relocation.text_offset,
                    layout.text_address + relocation.text_offset as u64,
                    symbol_address,
                )?;
            }
            RelocationKind::Aarch64PageOffset12 => {
                patch_aarch64_add_page_offset(
                    &mut image.text,
                    relocation.text_offset,
                    symbol_address,
                )?;
            }
            RelocationKind::Aarch64Branch26 => {
                patch_aarch64_branch26(
                    &mut image.text,
                    relocation.text_offset,
                    layout.text_address + relocation.text_offset as u64,
                    symbol_address,
                )?;
            }
            RelocationKind::X86_64Absolute64 | RelocationKind::X86_64Relative32 => {
                return Err(Diagnostic::error(format!(
                    "{output_name} AArch64 image received x86_64 relocation"
                )));
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinalImageLayout {
    pub text_address: u64,
    pub data_address: u64,
    pub bss_address: u64,
}

fn symbol_handle(symbols: &Arena<FinalImageSymbol>, symbol_name: &str) -> FinalImageSymbolHandle {
    symbols
        .iter()
        .find(|(_, symbol)| symbol.name == symbol_name)
        .map(|(handle, _)| handle)
        .unwrap_or_else(Handle::invalid)
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
            "AArch64 ADRP relocation is out of range: {page_delta} page(s)"
        )));
    }

    let immediate = (page_delta as u32) & 0x1f_ffff;
    let immediate_low = immediate & 0b11;
    let immediate_high = (immediate >> 2) & 0x7ffff;
    let mut instruction = read_u32(text, offset)?;
    instruction &= !((0b11 << 29) | (0x7ffff << 5));
    instruction |= (immediate_low << 29) | (immediate_high << 5);
    write_u32(text, offset, instruction)
}

fn patch_aarch64_add_page_offset(
    text: &mut [u8],
    offset: usize,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let page_offset = (symbol_address & 0xfff) as u32;
    let mut instruction = read_u32(text, offset)?;
    instruction &= !(0xfff << 10);
    instruction |= page_offset << 10;
    write_u32(text, offset, instruction)
}

fn patch_aarch64_branch26(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let byte_delta = symbol_address as i64 - instruction_address as i64;
    if byte_delta % 4 != 0 {
        return Err(Diagnostic::error(
            "AArch64 branch relocation target is not instruction-aligned",
        ));
    }
    let immediate = byte_delta / 4;
    if !(-(1 << 25)..(1 << 25)).contains(&immediate) {
        return Err(Diagnostic::error(format!(
            "AArch64 branch relocation is out of range: {immediate} instruction(s)"
        )));
    }

    let mut instruction = read_u32(text, offset)?;
    instruction &= !0x03ff_ffff;
    instruction |= (immediate as u32) & 0x03ff_ffff;
    write_u32(text, offset, instruction)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("AArch64 relocation offset overflow"))?;
    let Some(slice) = bytes.get(offset..end) else {
        return Err(Diagnostic::error(format!(
            "AArch64 relocation offset {offset} is outside text section"
        )));
    };

    Ok(u32::from_le_bytes(
        slice.try_into().expect("u32 relocation slice has length 4"),
    ))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("AArch64 relocation offset overflow"))?;
    let Some(slice) = bytes.get_mut(offset..end) else {
        return Err(Diagnostic::error(format!(
            "AArch64 relocation offset {offset} is outside text section"
        )));
    };

    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn final_image_section(section_name: &str) -> FinalImageSection {
    match section_name {
        ".text" | "__TEXT,__text" => FinalImageSection::Text,
        ".data" | "__DATA,__data" => FinalImageSection::Data,
        ".bss" | "__DATA,__bss" => FinalImageSection::Bss,
        _ => FinalImageSection::None,
    }
}

fn section_size(native_plan: &NativePlan, kind: SectionKind) -> usize {
    native_plan
        .object
        .sections
        .iter()
        .find(|(_, section)| section.kind == kind)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn section_alignment(native_plan: &NativePlan, kind: SectionKind) -> usize {
    native_plan
        .object
        .sections
        .iter()
        .find(|(_, section)| section.kind == kind)
        .map(|(_, section)| section.alignment)
        .unwrap_or(1)
}
