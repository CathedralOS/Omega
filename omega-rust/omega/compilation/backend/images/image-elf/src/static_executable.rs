/*
We emit static `ET_EXEC` ELF64 executables (`emit_elf_executable`) with a
64-byte ELF header, two `PT_LOAD` program headers, and no section headers,
refusing any image that retains a referenced dynamic import.
*/

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalImage, PlacedDataRegionInventory, apply_relocations,
    place_data_regions, place_executable_regions,
};

mod headers {
    use crate::bytes::{write_u16, write_u32, write_u64};
    use crate::constants::{
        ELF_HEADER_SIZE, IMAGE_BASE, PAGE_SIZE, PROGRAM_HEADER_COUNT, PROGRAM_HEADER_SIZE,
    };

    pub(crate) fn write_elf_header(
        bytes: &mut Vec<u8>,
        machine: u16,
        entry_address: u64,
        text_offset: usize,
        data_offset: usize,
    ) {
        bytes.extend([0x7f, b'E', b'L', b'F']);
        bytes.push(2);
        bytes.push(1);
        bytes.push(1);
        bytes.push(0);
        bytes.extend([0; 8]);
        write_u16(bytes, 2);
        write_u16(bytes, machine);
        write_u32(bytes, 1);
        write_u64(bytes, entry_address);
        write_u64(bytes, ELF_HEADER_SIZE as u64);
        write_u64(bytes, 0);
        write_u32(bytes, 0);
        write_u16(bytes, ELF_HEADER_SIZE as u16);
        write_u16(bytes, PROGRAM_HEADER_SIZE as u16);
        write_u16(bytes, PROGRAM_HEADER_COUNT as u16);
        write_u16(bytes, 0);
        write_u16(bytes, 0);
        write_u16(bytes, 0);

        debug_assert_eq!(bytes.len(), ELF_HEADER_SIZE);
        debug_assert!(text_offset >= bytes.len());
        debug_assert!(data_offset > text_offset);
    }

    pub(crate) fn write_text_program_header(
        bytes: &mut Vec<u8>,
        text_offset: usize,
        text_size: usize,
    ) {
        write_program_header(
            bytes,
            5,
            0,
            IMAGE_BASE,
            u64::try_from(text_offset + text_size).expect("ELF text segment size overflow"),
            u64::try_from(text_offset + text_size).expect("ELF text memory size overflow"),
        );
    }

    pub(crate) fn write_data_program_header(
        bytes: &mut Vec<u8>,
        data_offset: usize,
        data_size: usize,
        data_memory_size: u64,
    ) {
        write_program_header(
            bytes,
            6,
            u64::try_from(data_offset).expect("ELF data offset overflow"),
            IMAGE_BASE + data_offset as u64,
            u64::try_from(data_size).expect("ELF data segment size overflow"),
            data_memory_size,
        );
    }

    fn write_program_header(
        bytes: &mut Vec<u8>,
        flags: u32,
        offset: u64,
        virtual_address: u64,
        file_size: u64,
        memory_size: u64,
    ) {
        write_u32(bytes, 1);
        write_u32(bytes, flags);
        write_u64(bytes, offset);
        write_u64(bytes, virtual_address);
        write_u64(bytes, virtual_address);
        write_u64(bytes, file_size);
        write_u64(bytes, memory_size);
        write_u64(bytes, PAGE_SIZE as u64);
    }
}

mod layout {
    pub(crate) use image::{align_to, align_to_u64};
}

mod sections {
    use crate::constants::{
        ELF_HEADER_SIZE, IMAGE_BASE, PAGE_SIZE, PROGRAM_HEADER_COUNT, PROGRAM_HEADER_SIZE,
    };
    use crate::static_executable::layout::{align_to, align_to_u64};
    use image::{FinalImage, FinalImageLayout};

    pub(crate) struct ElfSections {
        pub(crate) text_offset: usize,
        pub(crate) data_offset: usize,
        pub(crate) text_address: u64,
        pub(crate) data_address: u64,
        pub(crate) bss_address: u64,
        pub(crate) data_memory_size: u64,
    }

    impl ElfSections {
        pub(crate) fn final_image_layout(&self) -> FinalImageLayout {
            FinalImageLayout {
                text_address: self.text_address,
                data_address: self.data_address,
                bss_address: self.bss_address,
            }
        }
    }

    pub(crate) fn plan_elf_sections(image: &FinalImage) -> ElfSections {
        let text_offset = align_to(
            ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * PROGRAM_HEADER_COUNT,
            PAGE_SIZE,
        );
        let data_offset = align_to(text_offset + image.memory.text.len(), PAGE_SIZE);
        let text_address = IMAGE_BASE + text_offset as u64;
        let data_address = IMAGE_BASE + data_offset as u64;
        let bss_address = align_to_u64(
            data_address + image.memory.data.len() as u64,
            image.memory.bss_alignment as u64,
        );
        let data_memory_size = (bss_address - data_address)
            .checked_add(image.memory.bss_size as u64)
            .expect("ELF data memory size overflow");

        ElfSections {
            text_offset,
            data_offset,
            text_address,
            data_address,
            bss_address,
            data_memory_size,
        }
    }

    #[cfg(test)]
    mod tests {}
}

use crate::entry_symbol::elf_entry_address;
use crate::imports::{ElfImportLocator, canonical_referenced_imports};
use headers::{write_data_program_header, write_elf_header, write_text_program_header};
use sections::plan_elf_sections;

const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

pub fn emit_elf_executable(mut image: FinalImage) -> Result<ExecutableImageOutput, Diagnostic> {
    let (machine, format) = match (image.target.object_format, image.target.architecture) {
        (target::ObjectFormat::Elf, target::Architecture::Aarch64) => {
            (EM_AARCH64, "elf64-aarch64-executable")
        }
        (target::ObjectFormat::Elf, target::Architecture::X86_64) => {
            (EM_X86_64, "elf64-x86-64-executable")
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "ELF64 executable emission cannot supply an image whose target is {:?}",
                image.target,
            )));
        }
    };
    let imports = canonical_referenced_imports(&image)?;
    if let Some(import) = imports.first() {
        let message = match &import.locator {
            ElfImportLocator::StringBackedBootstrap { library, symbol } => format!(
                "ELF direct image relocation references unknown symbol `{symbol}`; canonical dynamic import request names library `{library}` at {} relocation site(s), but ELF loader binding is not implemented",
                import.relocations.len(),
            ),
            ElfImportLocator::Component { locator, .. } => format!(
                "component ELF image {} requires the selected loader and separate-image publication; static emission cannot resolve this slot",
                locator.image_filename(),
            ),
            ElfImportLocator::Versioned {
                target_profile,
                compatibility_report_identity,
                object,
                symbol,
                version,
            } => format!(
                "versioned ELF foreign locator report 0x{compatibility_report_identity:016x} for target `{}` reached final emission with object {}, symbol {}, version {}, and {} exact relocation site(s); runnable dynamic ELF emission remains fail-closed before image mutation because no target-owned ELF loader plan carries the exact PT_INTERP bytes",
                target_profile.target_name(),
                hex_bytes(object),
                hex_bytes(symbol),
                hex_bytes(version),
                import.relocations.len(),
            ),
        };
        return Err(Diagnostic::error(message));
    }
    let sections = plan_elf_sections(&image);
    let layout = sections.final_image_layout();
    let entry_address = elf_entry_address(&image, sections.text_address)?;

    apply_relocations(&mut image, &layout, "ELF direct image")?;
    let executable_regions = place_executable_regions(&image, layout)?;
    let data_regions = place_data_regions(&image, layout)?;

    let mut bytes = Vec::with_capacity(sections.data_offset + image.memory.data.len());
    write_elf_header(
        &mut bytes,
        machine,
        entry_address,
        sections.text_offset,
        sections.data_offset,
    );
    write_text_program_header(&mut bytes, sections.text_offset, image.memory.text.len());
    write_data_program_header(
        &mut bytes,
        sections.data_offset,
        image.memory.data.len(),
        sections.data_memory_size,
    );
    bytes.resize(sections.text_offset, 0);
    bytes.extend(&image.memory.text);
    bytes.resize(sections.data_offset, 0);
    bytes.extend(&image.memory.data);

    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        final_import_data_bytes: Vec::new(),
        bytes,
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions: PlacedDataRegionInventory::empty(),
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len().saturating_mul(2).saturating_add(2));
    rendered.push_str("0x");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use crate::emit_elf_executable;
    use arena::Handle;
    use image::{FinalImage, FinalImageSection, FinalImageSymbol};

    #[test]
    fn emits_entry_address_from_final_image_entry_symbol() {
        let mut image = FinalImage::with_capacity(
            target::NativeTarget::linux_arm64(),
            image::FinalImageMemory {
                text: vec![0; 16],
                ..Default::default()
            },
            Handle::invalid(),
            0,
            0,
            0,
        );
        let entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".into(),
            section: FinalImageSection::Text,
            offset: 4,
            size: 4,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.entry_symbol = entry_symbol;

        let output = emit_elf_executable(image).expect("ELF image should emit");
        let entry_bytes: [u8; 8] = output.bytes[24..32].try_into().unwrap();

        assert_eq!(u64::from_le_bytes(entry_bytes), 0x401004);
        assert_eq!(output.format, "elf64-aarch64-executable");
        assert_eq!(&output.bytes[18..20], &183u16.to_le_bytes(), "e_machine");
        assert_eq!(output.executable_regions.text_address, 0x401000);
        assert_eq!(output.executable_regions.text_byte_count, 16);
        assert_eq!(output.executable_regions.unclassified_gaps.len(), 1);

        assert!(
            emit_elf_executable(FinalImage::with_capacity(
                target::NativeTarget::macos_arm64(),
                image::FinalImageMemory::default(),
                Handle::invalid(),
                0,
                0,
                0,
            ))
            .is_err()
        );
    }
}
