//! Mach-O dyld rebase opcodes for initialized internal pointers.
//!
//! Architecture relocation application writes each `Absolute64` target at the
//! executable's preferred address.  A PIE image also has to name that exact
//! pointer site to dyld so the loader can add the image slide.

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
const DATA_SEGMENT_INDEX: u8 = 2;
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
    ) -> Result<(), Diagnostic> {
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
        }
        Ok(())
    }
}

pub(crate) fn macho_rebase_info(image: &FinalImage) -> Result<MachoRebaseInfo, Diagnostic> {
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
        bytes.push(REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB | DATA_SEGMENT_INDEX);
        write_uleb128(&mut bytes, site.offset as u64);
        bytes.push(REBASE_OPCODE_DO_REBASE_IMM_TIMES | 1);
    }
    bytes.push(REBASE_OPCODE_DONE);
    Ok(MachoRebaseInfo { bytes, sites })
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

#[cfg(test)]
mod tests {
    use super::macho_rebase_info;
    use arena::Handle;
    use image::{
        FinalImage, FinalImageLayout, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
    };
    use object_file::{RelocationKind, SymbolKind};

    #[test]
    fn data_absolute_relocations_become_exact_dyld_pointer_sites() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                data: vec![0; 0x98],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            2,
        );
        let function = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "private-realization".to_owned(),
            section: FinalImageSection::Text,
            offset: 4,
            size: 4,
            kind: SymbolKind::Function,
        });
        for offset in [0x90, 0x08] {
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Data,
                    offset,
                    byte_width: 8,
                    symbol_handle: function,
                    addend: 0,
                    kind: RelocationKind::Absolute64,
                });
        }

        assert_eq!(
            macho_rebase_info(&image).expect("valid rebase sites").bytes,
            vec![0x11, 0x22, 0x08, 0x51, 0x22, 0x90, 0x01, 0x51, 0x00]
        );
    }

    #[test]
    fn rejects_non_data_or_malformed_absolute_relocations() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                text: vec![0; 8],
                data: vec![0; 8],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            1,
        );
        let function = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "private-realization".to_owned(),
            section: FinalImageSection::Text,
            offset: 0,
            size: 8,
            kind: SymbolKind::Function,
        });
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
        assert!(macho_rebase_info(&image).is_err());

        image
            .relocation_table
            .relocations
            .get_mut(relocation)
            .section = FinalImageSection::Data;
        image
            .relocation_table
            .relocations
            .get_mut(relocation)
            .byte_width = 4;
        assert!(macho_rebase_info(&image).is_err());

        image
            .relocation_table
            .relocations
            .get_mut(relocation)
            .byte_width = 8;
        image
            .relocation_table
            .relocations
            .get_mut(relocation)
            .offset = 1;
        assert!(macho_rebase_info(&image).is_err());
    }

    #[test]
    fn preferred_pointer_replay_rejects_mutated_patched_bytes() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                text: vec![0; 8],
                data: vec![0; 8],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            1,
        );
        let function = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "private-realization".to_owned(),
            section: FinalImageSection::Text,
            offset: 4,
            size: 4,
            kind: SymbolKind::Function,
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 0,
                byte_width: 8,
                symbol_handle: function,
                addend: 3,
                kind: RelocationKind::Absolute64,
            });
        let info = macho_rebase_info(&image).expect("valid internal pointer");
        let layout = FinalImageLayout {
            text_address: 0x1_0000_1000,
            data_address: 0x1_0000_4000,
            bss_address: 0x1_0000_5000,
        };
        image::apply_aarch64_relocations(&mut image, &layout, "test Mach-O")
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
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                data: vec![0; 8],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            2,
        );
        let function = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "private-realization".to_owned(),
            section: FinalImageSection::Text,
            offset: 0,
            size: 4,
            kind: SymbolKind::Function,
        });
        let first = image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 0,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        assert!(macho_rebase_info(&image).is_err());

        image
            .relocation_table
            .relocations
            .get_mut(first)
            .symbol_handle = function;
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 0,
                byte_width: 8,
                symbol_handle: function,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        assert!(macho_rebase_info(&image).is_err());
    }
}
